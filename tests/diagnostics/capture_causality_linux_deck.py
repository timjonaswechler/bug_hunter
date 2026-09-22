#!/usr/bin/env python3
"""Create and position one owned X11 occluder window.

Commands on stdin are limited to the supplied owned target XID. The helper
never searches for or manipulates unrelated windows.
"""

import argparse
import ctypes
import ctypes.util
import json
import os
from pathlib import Path
import sys
import time


Window = ctypes.c_ulong
Display = ctypes.c_void_p


class XWindowAttributes(ctypes.Structure):
    _fields_ = [
        ("x", ctypes.c_int),
        ("y", ctypes.c_int),
        ("width", ctypes.c_int),
        ("height", ctypes.c_int),
        ("border_width", ctypes.c_int),
        ("depth", ctypes.c_int),
        ("visual", ctypes.c_void_p),
        ("root", Window),
        ("class_", ctypes.c_int),
        ("bit_gravity", ctypes.c_int),
        ("win_gravity", ctypes.c_int),
        ("backing_store", ctypes.c_int),
        ("backing_planes", ctypes.c_ulong),
        ("backing_pixel", ctypes.c_ulong),
        ("save_under", ctypes.c_int),
        ("colormap", ctypes.c_ulong),
        ("map_installed", ctypes.c_int),
        ("map_state", ctypes.c_int),
        ("all_event_masks", ctypes.c_long),
        ("your_event_mask", ctypes.c_long),
        ("do_not_propagate_mask", ctypes.c_long),
        ("override_redirect", ctypes.c_int),
        ("screen", ctypes.c_void_p),
    ]


class XSetWindowAttributes(ctypes.Structure):
    _fields_ = [
        ("background_pixmap", ctypes.c_ulong),
        ("background_pixel", ctypes.c_ulong),
        ("border_pixmap", ctypes.c_ulong),
        ("border_pixel", ctypes.c_ulong),
        ("bit_gravity", ctypes.c_int),
        ("win_gravity", ctypes.c_int),
        ("backing_store", ctypes.c_int),
        ("backing_planes", ctypes.c_ulong),
        ("backing_pixel", ctypes.c_ulong),
        ("save_under", ctypes.c_int),
        ("event_mask", ctypes.c_long),
        ("do_not_propagate_mask", ctypes.c_long),
        ("override_redirect", ctypes.c_int),
        ("colormap", ctypes.c_ulong),
        ("cursor", ctypes.c_ulong),
    ]


class X11:
    def __init__(self):
        library = ctypes.util.find_library("X11")
        if not library:
            raise RuntimeError("libX11 is required for the X11 deck window")
        self.lib = ctypes.CDLL(library)
        self._signatures()
        self.display = self.lib.XOpenDisplay(None)
        if not self.display:
            raise RuntimeError("XOpenDisplay failed")
        self.screen = self.lib.XDefaultScreen(self.display)
        self.root = self.lib.XRootWindow(self.display, self.screen)
        self.screen_width = self.lib.XDisplayWidth(self.display, self.screen)
        self.screen_height = self.lib.XDisplayHeight(self.display, self.screen)

    def _signatures(self):
        lib = self.lib
        lib.XOpenDisplay.argtypes = [ctypes.c_char_p]
        lib.XOpenDisplay.restype = Display
        lib.XDefaultScreen.argtypes = [Display]
        lib.XDefaultScreen.restype = ctypes.c_int
        lib.XRootWindow.argtypes = [Display, ctypes.c_int]
        lib.XRootWindow.restype = Window
        lib.XDisplayWidth.argtypes = [Display, ctypes.c_int]
        lib.XDisplayHeight.argtypes = [Display, ctypes.c_int]
        lib.XCreateSimpleWindow.argtypes = [
            Display, Window, ctypes.c_int, ctypes.c_int, ctypes.c_uint, ctypes.c_uint,
            ctypes.c_uint, ctypes.c_ulong, ctypes.c_ulong,
        ]
        lib.XCreateSimpleWindow.restype = Window
        lib.XChangeWindowAttributes.argtypes = [
            Display, Window, ctypes.c_ulong, ctypes.POINTER(XSetWindowAttributes),
        ]
        lib.XStoreName.argtypes = [Display, Window, ctypes.c_char_p]
        lib.XMoveResizeWindow.argtypes = [
            Display, Window, ctypes.c_int, ctypes.c_int, ctypes.c_uint, ctypes.c_uint,
        ]
        lib.XMapRaised.argtypes = [Display, Window]
        lib.XRaiseWindow.argtypes = [Display, Window]
        lib.XSetInputFocus.argtypes = [Display, Window, ctypes.c_int, ctypes.c_ulong]
        lib.XGetWindowAttributes.argtypes = [Display, Window, ctypes.POINTER(XWindowAttributes)]
        lib.XGetWindowAttributes.restype = ctypes.c_int
        lib.XTranslateCoordinates.argtypes = [
            Display, Window, Window, ctypes.c_int, ctypes.c_int,
            ctypes.POINTER(ctypes.c_int), ctypes.POINTER(ctypes.c_int), ctypes.POINTER(Window),
        ]
        lib.XTranslateCoordinates.restype = ctypes.c_int
        lib.XSync.argtypes = [Display, ctypes.c_int]
        lib.XUnmapWindow.argtypes = [Display, Window]
        lib.XDestroyWindow.argtypes = [Display, Window]
        lib.XCloseDisplay.argtypes = [Display]

    def geometry(self, window):
        attributes = XWindowAttributes()
        if not self.lib.XGetWindowAttributes(self.display, window, ctypes.byref(attributes)):
            raise RuntimeError(f"XGetWindowAttributes failed for {int(window)}")
        x = ctypes.c_int()
        y = ctypes.c_int()
        child = Window()
        if not self.lib.XTranslateCoordinates(
            self.display, window, self.root, 0, 0,
            ctypes.byref(x), ctypes.byref(y), ctypes.byref(child),
        ):
            raise RuntimeError(f"XTranslateCoordinates failed for {int(window)}")
        return (x.value, y.value, attributes.width, attributes.height, attributes.map_state)

    def close(self):
        if self.display:
            self.lib.XCloseDisplay(self.display)
            self.display = None


def overlap(a, b):
    ax, ay, aw, ah = a[:4]
    bx, by, bw, bh = b[:4]
    return max(0, min(ax + aw, bx + bw) - max(ax, bx)) * max(
        0, min(ay + ah, by + bh) - max(ay, by)
    )


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--target-xid", type=int, required=True)
    parser.add_argument("--log-dir", type=Path, required=True)
    args = parser.parse_args()

    args.log_dir.mkdir(parents=True, exist_ok=True)
    log_path = args.log_dir / f"deck_x11-{os.getpid()}.jsonl"
    log = log_path.open("a", buffering=1)

    def emit(event, **fields):
        row = {
            "source": "deck_x11",
            "pid": os.getpid(),
            "unix_time_ns": time.time_ns(),
            "event": event,
            **fields,
        }
        line = json.dumps(row, separators=(",", ":")) + "\n"
        log.write(line)
        log.flush()
        os.fsync(log.fileno())
        print(line, end="", flush=True)
        return row

    x11 = X11()
    target = Window(args.target_xid)
    deck = x11.lib.XCreateSimpleWindow(
        x11.display, x11.root, 0, 0, 4, 4, 0, 0, 0,
    )
    attributes = XSetWindowAttributes()
    attributes.override_redirect = 1
    x11.lib.XChangeWindowAttributes(x11.display, deck, 1 << 9, ctypes.byref(attributes))
    x11.lib.XStoreName(x11.display, deck, b"Woodpecker Linux capture deck")
    x11.lib.XSync(x11.display, 0)
    emit(
        "deck_ready",
        target_xid=args.target_xid,
        deck_xid=int(deck),
        screen_rect=[0, 0, x11.screen_width, x11.screen_height],
    )

    try:
        for line in sys.stdin:
            words = line.split()
            if not words:
                continue
            command = words[0]
            if command == "quit" and len(words) == 1:
                x11.lib.XUnmapWindow(x11.display, deck)
                x11.lib.XSync(x11.display, 0)
                emit("deck_state", state="quit", target_xid=args.target_xid, deck_xid=int(deck))
                break
            if command not in {"small", "partial", "cover"} or len(words) != 6:
                raise ValueError(f"invalid deck command: {line.rstrip()}")
            placement_id = words[1]
            target_rect = tuple(int(value) for value in words[2:])
            tx, ty, tw, th = target_rect
            if tw <= 0 or th <= 0:
                raise ValueError(f"invalid target rectangle: {target_rect}")
            if command == "small":
                candidates = [
                    (x11.screen_width - 6, x11.screen_height - 6, 4, 4),
                    (2, 2, 4, 4),
                    (x11.screen_width - 6, 2, 4, 4),
                    (2, x11.screen_height - 6, 4, 4),
                ]
                geometry = next((item for item in candidates if overlap(target_rect, item) == 0), None)
                if geometry is None:
                    raise RuntimeError("no non-overlapping focus-deck position on the X11 screen")
            elif command == "partial":
                left = tx + tw // 2
                geometry = (left, ty - 8, tx + tw - left + 8, th + 16)
            else:
                geometry = (tx - 8, ty - 8, tw + 16, th + 16)

            x11.lib.XRaiseWindow(x11.display, target)
            x11.lib.XMoveResizeWindow(x11.display, deck, *geometry)
            x11.lib.XMapRaised(x11.display, deck)
            x11.lib.XRaiseWindow(x11.display, deck)
            x11.lib.XSetInputFocus(x11.display, deck, 1, 0)
            x11.lib.XSync(x11.display, 0)
            actual_target = x11.geometry(target)
            actual_deck = x11.geometry(deck)
            emit(
                "deck_state",
                state=command,
                placement_id=placement_id,
                target_xid=args.target_xid,
                deck_xid=int(deck),
                target_rect=list(actual_target[:4]),
                target_map_state=actual_target[4],
                deck_rect=list(actual_deck[:4]),
                deck_map_state=actual_deck[4],
                intersection_area=overlap(actual_target, actual_deck),
                target_area=actual_target[2] * actual_target[3],
            )
    finally:
        try:
            x11.lib.XUnmapWindow(x11.display, deck)
            x11.lib.XDestroyWindow(x11.display, deck)
            x11.lib.XSync(x11.display, 0)
        finally:
            emit("deck_cleanup", deck_xid=int(deck))
            x11.close()
            log.close()


if __name__ == "__main__":
    main()
