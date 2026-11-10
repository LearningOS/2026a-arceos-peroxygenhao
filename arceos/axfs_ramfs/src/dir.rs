use alloc::collections::BTreeMap;
use alloc::sync::{Arc, Weak};
use alloc::{string::String, vec::Vec};

use axfs_vfs::{VfsDirEntry, VfsNodeAttr, VfsNodeOps, VfsNodeRef, VfsNodeType};
use axfs_vfs::{VfsError, VfsResult};
use spin::RwLock;

use crate::file::FileNode;

/// The directory node in the RAM filesystem.
///
/// It implements [`axfs_vfs::VfsNodeOps`].
pub struct DirNode {
    this: Weak<DirNode>,
    parent: RwLock<Weak<dyn VfsNodeOps>>,
    children: RwLock<BTreeMap<String, VfsNodeRef>>,
}

impl DirNode {
    pub(super) fn new(parent: Option<Weak<dyn VfsNodeOps>>) -> Arc<Self> {
        Arc::new_cyclic(|this| Self {
            this: this.clone(),
            parent: RwLock::new(parent.unwrap_or_else(|| Weak::<Self>::new())),
            children: RwLock::new(BTreeMap::new()),
        })
    }

    pub(super) fn set_parent(&self, parent: Option<&VfsNodeRef>) {
        *self.parent.write() = parent.map_or(Weak::<Self>::new() as _, Arc::downgrade);
    }

    /// Returns a string list of all entries in this directory.
    pub fn get_entries(&self) -> Vec<String> {
        self.children.read().keys().cloned().collect()
    }

    /// Checks whether a node with the given name exists in this directory.
    pub fn exist(&self, name: &str) -> bool {
        self.children.read().contains_key(name)
    }

    /// Creates a new node with the given name and type in this directory.
    pub fn create_node(&self, name: &str, ty: VfsNodeType) -> VfsResult {
        if self.exist(name) {
            log::error!("AlreadyExists {}", name);
            return Err(VfsError::AlreadyExists);
        }
        let node: VfsNodeRef = match ty {
            VfsNodeType::File => Arc::new(FileNode::new()),
            VfsNodeType::Dir => Self::new(Some(self.this.clone())),
            _ => return Err(VfsError::Unsupported),
        };
        self.children.write().insert(name.into(), node);
        Ok(())
    }

    /// Removes a node by the given name in this directory.
    pub fn remove_node(&self, name: &str) -> VfsResult {
        let mut children = self.children.write();
        let node = children.get(name).ok_or(VfsError::NotFound)?;
        if let Some(dir) = node.as_any().downcast_ref::<DirNode>() {
            if !dir.children.read().is_empty() {
                return Err(VfsError::DirectoryNotEmpty);
            }
        }
        children.remove(name);
        Ok(())
    }

    /// Renames `src_name` to `dst_name`, both located directly in this directory.
    ///
    /// Fails if `src_name` does not exist or if `dst_name` is already taken.
    pub fn rename_node(&self, src_name: &str, dst_name: &str) -> VfsResult {
        if src_name == dst_name {
            return if self.exist(src_name) {
                Ok(())
            } else {
                Err(VfsError::NotFound)
            };
        }
        let mut children = self.children.write();
        if !children.contains_key(src_name) {
            return Err(VfsError::NotFound);
        }
        if children.contains_key(dst_name) {
            return Err(VfsError::AlreadyExists);
        }
        let node = children.remove(src_name).ok_or(VfsError::NotFound)?;
        children.insert(String::from(dst_name), node);
        Ok(())
    }

    /// Resolves `path` to a sub-directory of `self`.
    ///
    /// An empty path (or a path made of `.` only) refers to `self`.
    fn lookup_dir(&self, path: &str) -> VfsResult<Arc<DirNode>> {
        let mut cur = self.this.upgrade().ok_or(VfsError::NotFound)?;
        for comp in path.split('/') {
            if comp.is_empty() || comp == "." {
                continue;
            }
            let node: VfsNodeRef = if comp == ".." {
                cur.parent().ok_or(VfsError::NotFound)?
            } else {
                cur.children
                    .read()
                    .get(comp)
                    .cloned()
                    .ok_or(VfsError::NotFound)?
            };
            cur = dir_node_of(node)?;
        }
        Ok(cur)
    }
}

/// Casts a VFS node to the RAM filesystem directory node behind it.
fn dir_node_of(node: VfsNodeRef) -> VfsResult<Arc<DirNode>> {
    node.as_any()
        .downcast_ref::<DirNode>()
        .and_then(|dir| dir.this.upgrade())
        .ok_or(VfsError::NotADirectory)
}

impl VfsNodeOps for DirNode {
    fn get_attr(&self) -> VfsResult<VfsNodeAttr> {
        Ok(VfsNodeAttr::new_dir(4096, 0))
    }

    fn parent(&self) -> Option<VfsNodeRef> {
        self.parent.read().upgrade()
    }

    fn lookup(self: Arc<Self>, path: &str) -> VfsResult<VfsNodeRef> {
        let (name, rest) = split_path(path);
        let node = match name {
            "" | "." => Ok(self.clone() as VfsNodeRef),
            ".." => self.parent().ok_or(VfsError::NotFound),
            _ => self
                .children
                .read()
                .get(name)
                .cloned()
                .ok_or(VfsError::NotFound),
        }?;

        if let Some(rest) = rest {
            node.lookup(rest)
        } else {
            Ok(node)
        }
    }

    fn read_dir(&self, start_idx: usize, dirents: &mut [VfsDirEntry]) -> VfsResult<usize> {
        let children = self.children.read();
        let mut children = children.iter().skip(start_idx.max(2) - 2);
        for (i, ent) in dirents.iter_mut().enumerate() {
            match i + start_idx {
                0 => *ent = VfsDirEntry::new(".", VfsNodeType::Dir),
                1 => *ent = VfsDirEntry::new("..", VfsNodeType::Dir),
                _ => {
                    if let Some((name, node)) = children.next() {
                        *ent = VfsDirEntry::new(name, node.get_attr().unwrap().file_type());
                    } else {
                        return Ok(i);
                    }
                }
            }
        }
        Ok(dirents.len())
    }

    fn create(&self, path: &str, ty: VfsNodeType) -> VfsResult {
        log::debug!("create {:?} at ramfs: {}", ty, path);
        let (name, rest) = split_path(path);
        if let Some(rest) = rest {
            match name {
                "" | "." => self.create(rest, ty),
                ".." => self.parent().ok_or(VfsError::NotFound)?.create(rest, ty),
                _ => {
                    let subdir = self
                        .children
                        .read()
                        .get(name)
                        .ok_or(VfsError::NotFound)?
                        .clone();
                    subdir.create(rest, ty)
                }
            }
        } else if name.is_empty() || name == "." || name == ".." {
            Ok(()) // already exists
        } else {
            self.create_node(name, ty)
        }
    }

    fn remove(&self, path: &str) -> VfsResult {
        log::debug!("remove at ramfs: {}", path);
        let (name, rest) = split_path(path);
        if let Some(rest) = rest {
            match name {
                "" | "." => self.remove(rest),
                ".." => self.parent().ok_or(VfsError::NotFound)?.remove(rest),
                _ => {
                    let subdir = self
                        .children
                        .read()
                        .get(name)
                        .ok_or(VfsError::NotFound)?
                        .clone();
                    subdir.remove(rest)
                }
            }
        } else if name.is_empty() || name == "." || name == ".." {
            Err(VfsError::InvalidInput) // remove '.' or '..
        } else {
            self.remove_node(name)
        }
    }

    /// Renames a node inside this directory.
    ///
    /// Only renaming is supported, not moving: the destination must live in the
    /// same directory as the source. `dst_path` may still be an absolute path
    /// that starts with this filesystem's mount point (that is how
    /// `axfs::root::rename` reaches a mounted filesystem), so only its last
    /// component is used as the new name.
    fn rename(&self, src_path: &str, dst_path: &str) -> VfsResult {
        log::debug!("rename at ramfs: {} -> {}", src_path, dst_path);

        let (src_dir_path, src_name) = split_name(src_path);
        let (dst_dir_path, dst_name) = split_name(dst_path);
        for name in [src_name, dst_name] {
            if name.is_empty() || name == "." || name == ".." {
                return Err(VfsError::InvalidInput);
            }
        }

        let src_dir = self.lookup_dir(src_dir_path)?;
        // A destination directory that cannot be resolved from here belongs to
        // the mount point prefix of the caller; anything that resolves must be
        // the source directory itself, otherwise this would be a move.
        match self.lookup_dir(dst_dir_path) {
            Ok(dst_dir) if !Arc::ptr_eq(&dst_dir, &src_dir) => return Err(VfsError::Unsupported),
            _ => {}
        }

        src_dir.rename_node(src_name, dst_name)
    }

    axfs_vfs::impl_vfs_dir_default! {}
}

/// Splits a path into its parent path and its last component.
fn split_name(path: &str) -> (&str, &str) {
    let path = path.trim_end_matches('/');
    path.rfind('/')
        .map_or(("", path), |i| (&path[..i], &path[i + 1..]))
}

fn split_path(path: &str) -> (&str, Option<&str>) {
    let trimmed_path = path.trim_start_matches('/');
    trimmed_path.find('/').map_or((trimmed_path, None), |n| {
        (&trimmed_path[..n], Some(&trimmed_path[n + 1..]))
    })
}
