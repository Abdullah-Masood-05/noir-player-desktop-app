"""Fail if a Windows PE executable depends on the dynamic MSVC/Universal CRT.

Noir Player is built with `-C target-feature=+crt-static`, so noir_player.exe
must not import VCRUNTIME140*.dll, MSVCP140*.dll or api-ms-win-crt-*.dll.
Those DLLs are absent on clean Windows installs that lack the Visual C++
Redistributable, which makes the app fail to start.

Usage:
  python check_static_crt.py [--exe EXE]... [--msi MSI]... [--nsis SETUP_EXE]... [--expect EXE]

--msi and --nsis installers are unpacked (msiexec administrative install and
7-Zip) and every noir_player.exe inside them is checked as well. With
--expect, each packaged noir_player.exe must be byte-identical to that file.
Prints every imported DLL (normal and delay-load) and exits non-zero with a
GitHub `::error::` annotation on any forbidden import, missing tool or parse
failure.
"""

import argparse
import fnmatch
import os
import pathlib
import shutil
import struct
import subprocess
import sys
import tempfile

FORBIDDEN = ('vcruntime140*.dll', 'msvcp140*.dll', 'api-ms-win-crt-*.dll')


def fail(message):
    print(f'::error::{message}')
    sys.exit(1)


def read_cstring(data, offset):
    end = data.index(b'\0', offset)
    return data[offset:end].decode('ascii')


def imports(path):
    data = path.read_bytes()
    if data[:2] != b'MZ':
        raise ValueError('not an MZ executable')
    pe = struct.unpack_from('<I', data, 0x3C)[0]
    if data[pe:pe + 4] != b'PE\0\0':
        raise ValueError('invalid PE signature')
    sections_count = struct.unpack_from('<H', data, pe + 6)[0]
    optional_size = struct.unpack_from('<H', data, pe + 20)[0]
    optional = pe + 24
    magic = struct.unpack_from('<H', data, optional)[0]
    if magic == 0x20B:
        directories = optional + 112
    elif magic == 0x10B:
        directories = optional + 96
    else:
        raise ValueError(f'unknown optional header magic {magic:#x}')
    sections = []
    table = optional + optional_size
    for index in range(sections_count):
        entry = table + 40 * index
        virtual_size, virtual_address, raw_size, raw_offset = struct.unpack_from('<IIII', data, entry + 8)
        sections.append((virtual_address, max(virtual_size, raw_size), raw_offset))

    def to_offset(rva):
        for virtual_address, size, raw_offset in sections:
            if virtual_address <= rva < virtual_address + size:
                return rva - virtual_address + raw_offset
        raise ValueError(f'RVA {rva:#x} is outside every section')

    found = []
    # (data directory index, descriptor size, offset of the DLL name RVA)
    for directory, kind, size, name_field in ((1, 'import', 20, 12), (13, 'delay', 32, 4)):
        rva = struct.unpack_from('<I', data, directories + 8 * directory)[0]
        if not rva:
            continue
        offset = to_offset(rva)
        while True:
            descriptor = data[offset:offset + size]
            if len(descriptor) < size or not any(descriptor):
                break
            name_rva = struct.unpack_from('<I', descriptor, name_field)[0]
            found.append((kind, read_cstring(data, to_offset(name_rva))))
            offset += size
    return found


def check(path):
    """Return True when `path` has no forbidden imports. Exits on parse failure."""
    if not path.is_file():
        fail(f'{path}: executable not found, cannot verify CRT linkage')
    try:
        found = imports(path)
    except Exception as error:  # noqa: BLE001 - any parse failure must fail the job
        fail(f'{path}: cannot parse PE imports ({error})')
    print(f'Imports of {path} ({len(found)}):')
    for kind, name in found:
        print(f'  [{kind}] {name}')
    if 'kernel32.dll' not in [name.lower() for _, name in found]:
        fail(f'{path}: import table looks wrong (no KERNEL32.dll), refusing to pass')
    bad = sorted({name for _, name in found if any(fnmatch.fnmatch(name.lower(), p) for p in FORBIDDEN)})
    if bad:
        print(
            f'::error file={path}::{path.name} dynamically links the MSVC runtime '
            f'({", ".join(bad)}). It will fail with "VCRUNTIME140.dll was not found" on machines '
            'without the Visual C++ Redistributable. Build with -C target-feature=+crt-static '
            '([target.x86_64-pc-windows-msvc] rustflags in .cargo/config.toml) and make sure no '
            'RUSTFLAGS/CARGO_ENCODED_RUSTFLAGS environment variable overrides it.'
        )
        return False
    print(f'{path}: no dynamic MSVC/UCRT runtime imports.')
    return True


def seven_zip():
    for candidate in (shutil.which('7z'), r'C:\Program Files\7-Zip\7z.exe'):
        if candidate and pathlib.Path(candidate).is_file():
            return candidate
    fail('7-Zip (7z.exe) not found, cannot unpack the NSIS installer to verify CRT linkage')


def unpack(kind, installer, destination):
    installer = installer.resolve()
    if not installer.is_file():
        fail(f'{installer}: installer not found')
    destination.mkdir(parents=True)
    if kind == 'msi':
        log = destination.with_suffix('.log')
        command = ['msiexec.exe', '/a', str(installer), '/qn', f'TARGETDIR={destination}', '/l*v', str(log)]
    else:
        command = [seven_zip(), 'x', '-y', f'-o{destination}', str(installer)]
    result = subprocess.run(command)
    if result.returncode != 0:
        if kind == 'msi' and log.is_file():
            print(log.read_text(encoding='utf-16', errors='replace')[-4000:])
        fail(f'Unpacking {installer} failed with exit code {result.returncode}: {command}')
    found = sorted(path for path in destination.rglob('*') if path.name.lower() == 'noir_player.exe')
    if not found:
        for path in destination.rglob('*'):
            print(path.relative_to(destination))
        fail(f'No noir_player.exe found inside {installer}')
    return found


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--exe', action='append', default=[], type=pathlib.Path)
    parser.add_argument('--msi', action='append', default=[], type=pathlib.Path)
    parser.add_argument('--nsis', action='append', default=[], type=pathlib.Path)
    parser.add_argument('--expect', type=pathlib.Path)
    args = parser.parse_args()
    if not (args.exe or args.msi or args.nsis):
        fail('check_static_crt.py: nothing to check')
    executables = list(args.exe)
    packaged = []
    if args.msi or args.nsis:
        root = pathlib.Path(tempfile.mkdtemp(prefix='crt-check-', dir=os.environ.get('RUNNER_TEMP')))
        for index, (kind, installer) in enumerate([('msi', p) for p in args.msi] + [('nsis', p) for p in args.nsis]):
            for path in unpack(kind, installer, root / f'{kind}-{index}'):
                print(f'{installer} contains {path}')
                packaged.append(path)
    if args.expect:
        expected = args.expect.read_bytes()
        for path in packaged:
            if path.read_bytes() != expected:
                fail(f'{path} differs from {args.expect}; the installer does not ship the verified build')
    ok = all([check(path) for path in executables + packaged])
    sys.exit(0 if ok else 1)


if __name__ == '__main__':
    main()
