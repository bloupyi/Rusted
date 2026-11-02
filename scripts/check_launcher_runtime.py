#!/usr/bin/env python3
"""Reject external Visual C++ runtime imports before publishing the launcher."""
from pathlib import Path
import argparse
import re
import struct


def imports(body):
    def number(offset, fmt='<I'):
        try:
            return struct.unpack_from(fmt, body, offset)[0]
        except struct.error as error:
            raise ValueError('Truncated PE image') from error
    pe = number(60)
    if body[:2] != b'MZ' or body[pe:pe + 4] != b'PE\0\0':
        raise ValueError('Invalid PE image')
    optional = pe + 24
    if number(optional, '<H') != 0x20b:
        raise ValueError('Expected a 64-bit PE image')
    sections = optional + number(pe + 20, '<H')
    count = number(pe + 6, '<H')
    def offset(rva):
        for i in range(count):
            section = sections + 40 * i
            va, raw_size, raw = number(section + 12), number(section + 16), number(section + 20)
            if va <= rva < va + raw_size:
                result = raw + rva - va
                if result >= len(body):
                    break
                return result
        raise ValueError('Invalid PE import address')
    def name(rva):
        pos = offset(rva)
        end = body.find(b'\0', pos, min(pos + 256, len(body)))
        if end < 0:
            raise ValueError('Invalid PE import name')
        return body[pos:end].decode('ascii').lower()
    found = set()
    directories = number(optional + 108)
    for directory, stride, name_field in [(1, 20, 12), (13, 32, 4)]:
        if directories <= directory:
            continue
        rva, size = number(optional + 112 + 8 * directory), number(optional + 116 + 8 * directory)
        if not rva:
            continue
        start = offset(rva)
        for i in range(min(size // stride, 4096)):
            pos = start + stride * i
            if body[pos:pos + stride] == bytes(stride):
                break
            dll = number(pos + name_field)
            if directory == 13 and not number(pos) & 1:
                dll -= number(optional + 24, '<Q')
            found.add(name(dll))
        else:
            raise ValueError('Unterminated PE imports')
    return sorted(found)


def verify(path):
    dependencies = imports(Path(path).read_bytes())
    blocked = [dll for dll in dependencies if re.match(r'(vcruntime|msvcp|concrt|msvcr\d)', dll)]
    if blocked:
        raise ValueError('Launcher still needs an external Visual C++ runtime: ' + ', '.join(blocked))
    return dependencies


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('executable', type=Path)
    args = parser.parse_args()
    print('Launcher imports verified:', ', '.join(verify(args.executable)))
