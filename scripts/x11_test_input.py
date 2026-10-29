#!/usr/bin/env python3
"""Linux visual-test helper: real X11 events and screenshots, no game injection."""
import argparse
import ctypes as C
import os
from pathlib import Path
import time

class XImage(C.Structure):
    _fields_ = [("width", C.c_int), ("height", C.c_int), ("xoffset", C.c_int), ("format", C.c_int),
                ("data", C.c_void_p), ("byte_order", C.c_int), ("bitmap_unit", C.c_int),
                ("bitmap_bit_order", C.c_int), ("bitmap_pad", C.c_int), ("depth", C.c_int),
                ("bytes_per_line", C.c_int), ("bits_per_pixel", C.c_int),
                ("red_mask", C.c_ulong), ("green_mask", C.c_ulong), ("blue_mask", C.c_ulong)]

class Display:
    def __init__(self):
        self.x = C.CDLL("libX11.so.6")
        self.t = C.CDLL("libXtst.so.6")
        self.x.XOpenDisplay.argtypes = [C.c_char_p]; self.x.XOpenDisplay.restype = C.c_void_p
        self.d = self.x.XOpenDisplay(None)
        if not self.d:
            raise RuntimeError("No X11 display available; set DISPLAY")
        self.x.XDefaultRootWindow.argtypes = [C.c_void_p]; self.x.XDefaultRootWindow.restype = C.c_ulong
        self.root = self.x.XDefaultRootWindow(self.d)
        self.x.XFlush.argtypes = [C.c_void_p]
        self.x.XSync.argtypes = [C.c_void_p, C.c_int]
        self.x.XWarpPointer.argtypes = [C.c_void_p, C.c_ulong, C.c_ulong, C.c_int, C.c_int, C.c_uint, C.c_uint, C.c_int, C.c_int]
        self.x.XStringToKeysym.argtypes = [C.c_char_p]; self.x.XStringToKeysym.restype = C.c_ulong
        self.x.XKeysymToKeycode.argtypes = [C.c_void_p, C.c_ulong]; self.x.XKeysymToKeycode.restype = C.c_uint
        self.t.XTestFakeKeyEvent.argtypes = [C.c_void_p, C.c_uint, C.c_int, C.c_ulong]
        self.t.XTestFakeButtonEvent.argtypes = [C.c_void_p, C.c_uint, C.c_int, C.c_ulong]
    def click(self, x, y, button=1):
        self.x.XWarpPointer(self.d, 0, self.root, 0, 0, 0, 0, x, y)
        self.t.XTestFakeButtonEvent(self.d, button, 1, 0)
        self.t.XTestFakeButtonEvent(self.d, button, 0, 0)
        self.x.XSync(self.d, 0)
    def key(self, name, hold=0.05):
        code = self.x.XKeysymToKeycode(self.d, self.x.XStringToKeysym(name.encode()))
        if code == 0:
            raise ValueError("Unknown X11 key: " + name)
        self.t.XTestFakeKeyEvent(self.d, code, 1, 0); self.x.XFlush(self.d)
        time.sleep(hold)
        self.t.XTestFakeKeyEvent(self.d, code, 0, 0); self.x.XSync(self.d, 0)
    def move(self, x, y):
        self.t.XTestFakeMotionEvent.argtypes = [C.c_void_p, C.c_int, C.c_int, C.c_int, C.c_ulong]
        self.t.XTestFakeMotionEvent(self.d, 0, x, y, 0); self.x.XSync(self.d, 0)
    def button(self, button, hold):
        self.t.XTestFakeButtonEvent(self.d, button, 1, 0); self.x.XFlush(self.d)
        time.sleep(hold)
        self.t.XTestFakeButtonEvent(self.d, button, 0, 0); self.x.XSync(self.d, 0)
    def screenshot(self, path):
        from PIL import Image
        self.x.XDisplayWidth.argtypes = [C.c_void_p, C.c_int]; self.x.XDisplayWidth.restype = C.c_int
        self.x.XDisplayHeight.argtypes = [C.c_void_p, C.c_int]; self.x.XDisplayHeight.restype = C.c_int
        width = self.x.XDisplayWidth(self.d, 0); height = self.x.XDisplayHeight(self.d, 0)
        self.x.XGetImage.argtypes = [C.c_void_p, C.c_ulong, C.c_int, C.c_int, C.c_uint, C.c_uint, C.c_ulong, C.c_int]
        self.x.XGetImage.restype = C.POINTER(XImage)
        pointer = self.x.XGetImage(self.d, self.root, 0, 0, width, height, C.c_ulong(-1).value, 2)
        if not pointer:
            raise RuntimeError("XGetImage failed")
        try:
            image = pointer.contents
            if image.bits_per_pixel != 32 or image.red_mask != 0xff0000 or image.byte_order != 0:
                raise RuntimeError("Unsupported X11 pixel format for this test helper")
            raw = C.string_at(image.data, image.bytes_per_line * image.height)
            result = Image.frombytes("RGB", (image.width, image.height), raw, "raw", "BGRX", image.bytes_per_line, 1)
            path.parent.mkdir(parents=True, exist_ok=True); result.save(path)
        finally:
            self.x.XDestroyImage.argtypes = [C.POINTER(XImage)]; self.x.XDestroyImage(pointer)

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="action", required=True)
    capture = sub.add_parser("screenshot"); capture.add_argument("path", type=Path)
    click = sub.add_parser("click"); click.add_argument("x", type=int); click.add_argument("y", type=int); click.add_argument("--button", type=int, default=1)
    key = sub.add_parser("key"); key.add_argument("name"); key.add_argument("--hold", type=float, default=0.05)
    move = sub.add_parser("move"); move.add_argument("x", type=int); move.add_argument("y", type=int)
    button = sub.add_parser("button"); button.add_argument("number", type=int); button.add_argument("--hold", type=float, default=0.1)
    args = parser.parse_args(); display = Display()
    if args.action == "screenshot": display.screenshot(args.path)
    elif args.action == "click": display.click(args.x, args.y, args.button)
    elif args.action == "key": display.key(args.name, args.hold)
    elif args.action == "move": display.move(args.x, args.y)
    else: display.button(args.number, args.hold)

if __name__ == "__main__":
    main()
