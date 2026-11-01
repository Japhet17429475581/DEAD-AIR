"""Send the real WM_DELETE_WINDOW protocol in the disposable Xvfb test session.

Unlike xdotool windowclose (XDestroyWindow), this lets GTK/Tauri handle cleanup.
"""
import ctypes as c
import os
import sys

if not os.path.exists('/.dockerenv'):
    raise SystemExit('Container-only verification utility')

class Data(c.Union):
    _fields_ = [('l', c.c_long * 5)]

class ClientMessage(c.Structure):
    _fields_ = [('type', c.c_int), ('serial', c.c_ulong), ('send_event', c.c_int),
                ('display', c.c_void_p), ('window', c.c_ulong), ('message_type', c.c_ulong),
                ('format', c.c_int), ('data', Data)]

class Event(c.Union):
    _fields_ = [('client', ClientMessage), ('pad', c.c_long * 24)]

x = c.CDLL('libX11.so.6')
x.XOpenDisplay.argtypes = [c.c_char_p]
x.XOpenDisplay.restype = c.c_void_p
x.XInternAtom.argtypes = [c.c_void_p, c.c_char_p, c.c_int]
x.XInternAtom.restype = c.c_ulong
x.XSendEvent.argtypes = [c.c_void_p, c.c_ulong, c.c_int, c.c_long, c.POINTER(Event)]
x.XFlush.argtypes = [c.c_void_p]
x.XCloseDisplay.argtypes = [c.c_void_p]
display = x.XOpenDisplay(None)
if not display:
    raise SystemExit('Cannot open test display')
event = Event()
event.client.type = 33
event.client.display = display
event.client.window = int(sys.argv[1], 0)
event.client.message_type = x.XInternAtom(display, b'WM_PROTOCOLS', 0)
event.client.format = 32
event.client.data.l[0] = x.XInternAtom(display, b'WM_DELETE_WINDOW', 0)
if not x.XSendEvent(display, event.client.window, 0, 0, c.byref(event)):
    raise SystemExit('Window-close message failed')
x.XFlush(display)
x.XCloseDisplay(display)
