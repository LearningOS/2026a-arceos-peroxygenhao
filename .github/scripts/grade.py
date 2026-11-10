"""Run the six 2026s ArceOS experiments; preserve their output assertions."""

import os
import re
import subprocess
import sys

from course import COURSE, ROOT, record, run, write_result

MESSAGES = {
    "ramfs_rename": "[Ramfs-Rename]: ok!",
    "alt_alloc": "Bump tests run OK!",
    "support_hashmap": "Memory tests run OK!",
    "sys_map": "Read back content: hello, arceos!",
    "simple_hv": "Shutdown vm normally!",
}


def output_passed(name, text):
    lines = text.splitlines()
    if name == "print_with_color":
        tail = "\n".join(lines[-20:])
        plain = re.sub(r"\x1b\[[0-9;]*m", "", tail)
        return "\x1b[" in tail and "Hello, Arceos!" in plain
    if name in ("ramfs_rename", "alt_alloc", "support_hashmap"):
        return bool(lines) and MESSAGES[name] in lines[-1]
    return MESSAGES[name] in text


def prepare_images(kernel, output):
    images = ROOT / "tmp/images"
    images.mkdir(exist_ok=True)
    with (output / "prepare.log").open("a") as log:
        commands = [
            ["cargo", "build", "--locked", "-p", "origin", "--target", "riscv64gc-unknown-none-elf", "--release"],
            ["rust-objcopy", "--binary-architecture=riscv64", "--strip-all", "-O", "binary",
             "target/riscv64gc-unknown-none-elf/release/origin", str(images / "origin.bin")],
        ]
        for command in commands:
            code, _ = run(command, log, kernel, seconds=300)
            if code:
                raise RuntimeError(f"Boot image preparation failed, exit {code}; no score uploaded.")
        origin = (images / "origin.bin").read_bytes()
        with (images / "pflash.img").open("wb") as flash:
            flash.truncate(32 * 1024 * 1024)
            flash.write(b"pfld\x00\x00\x00\x01" + len(origin).to_bytes(4, "big"))
            flash.seek(16)
            flash.write(origin)
        with (images / "disk.img").open("wb") as disk:
            disk.truncate(64 * 1024 * 1024)
        for command in (["mkfs.fat", "-F", "32", "disk.img"],
                        ["mmd", "-i", "disk.img", "::/sbin"],
                        ["mcopy", "-i", "disk.img", str(images / "origin.bin"), "::/sbin/origin.bin"]):
            code, _ = run(command, log, kernel, seconds=60)
            if code:
                raise RuntimeError(f"Disk image preparation failed, exit {code}; no score uploaded.")


def main():
    os.chdir(ROOT)
    output = ROOT / "tmp/grade"
    output.mkdir(parents=True, exist_ok=True)
    os.environ["TMPDIR"] = str(ROOT / "tmp")
    kernel = ROOT / "arceos"
    # Keep generated images and compiler output inside this repository's tmp/.
    for name, relative in (("target", "tmp/arceos-target"),
                           ("disk.img", "tmp/images/disk.img"),
                           ("pflash.img", "tmp/images/pflash.img")):
        path = kernel / name
        destination = ROOT / relative
        if path.is_symlink() and path.resolve() == destination:
            continue
        if path.exists() or path.is_symlink():
            raise RuntimeError(f"{path} already exists; use a fresh CI checkout.")
        if name == "target":
            destination.mkdir(parents=True, exist_ok=True)
        path.symlink_to(destination)
    results = []
    for test in COURSE["tests"]:
        name = test["name"]
        print(f"::group::{name} (100 points)", flush=True)
        prepare_images(kernel, output)
        with (output / (name + ".log")).open("w") as log:
            if name in ("sys_map", "simple_hv"):
                payload = "mapfile_c" if name == "sys_map" else "skernel2"
                code, _ = run(["make", "-C", "payload/" + payload], log, kernel, seconds=300)
                if code:
                    raise RuntimeError(f"Payload preparation failed for {name}, exit {code}; no score uploaded.")
                binary = "mapfile" if name == "sys_map" else "skernel2"
                # mtools writes the same FAT /sbin path without requiring sudo mount.
                subprocess.run(["mcopy", "-i", "disk.img", f"payload/{payload}/{binary}", "::/sbin/"],
                               cwd=kernel, check=True)
            options = [f"A=exercises/{name}/"]
            if name in ("ramfs_rename", "sys_map", "simple_hv"):
                options.append("BLK=y")
            code, _ = run(["make", *options, "build"], log, kernel, seconds=600)
            passed = False
            if code == 0:
                code, text = run(["make", *options, "justrun"], log, kernel, seconds=90)
                passed = code == 0 and output_passed(name, text)
            results.append(record(test, code, passed))
        print("::endgroup::", flush=True)
    write_result(results)


if __name__ == "__main__":
    try:
        main()
    except (ValueError, RuntimeError, OSError, subprocess.CalledProcessError) as error:
        sys.exit(str(error))
