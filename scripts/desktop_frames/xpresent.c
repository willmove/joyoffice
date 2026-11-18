// Select X Present events on a test application's window and match its report to visible pixels.
#include <X11/Xlib.h>
#include <X11/Xutil.h>
#include <X11/extensions/Xpresent.h>
#include <inttypes.h>
#include <stdio.h>
#include <stdlib.h>
#include <time.h>

int main(int argc, char **argv) {
    if (argc != 6) return 2;
    Display *dpy = XOpenDisplay(NULL);
    if (!dpy) return 3;
    Window window = strtoul(argv[1], NULL, 0);
    int x = atoi(argv[2]), y = atoi(argv[3]), w = atoi(argv[4]), h = atoi(argv[5]);
    int opcode, event, error;
    if (w <= 0 || h <= 0 || !XPresentQueryExtension(dpy, &opcode, &event, &error)) return 4;
    XPresentSelectInput(dpy, window, PresentCompleteNotifyMask);
    XSync(dpy, False);
    puts("ready"); fflush(stdout);
    for (;;) {
        XEvent ev;
        XNextEvent(dpy, &ev);
        if (ev.type != GenericEvent || ev.xcookie.extension != opcode ||
            ev.xcookie.evtype != PresentCompleteNotify || !XGetEventData(dpy, &ev.xcookie)) continue;
        XPresentCompleteNotifyEvent *done = ev.xcookie.data;
        uint64_t ns = done->ust * 1000;
        int presented = done->window == window && done->kind == PresentCompleteKindPixmap &&
                        done->mode != PresentCompleteModeSkip;
        XFreeEventData(dpy, &ev.xcookie);
        if (!presented) continue;
        struct timespec observed;
        clock_gettime(CLOCK_MONOTONIC, &observed);
        uint64_t now = (uint64_t)observed.tv_sec * 1000000000 + observed.tv_nsec;
        if (now < ns || now - ns > 16000000) continue;
        XImage *image = XGetImage(dpy, window,x,y,w,h,AllPlanes,ZPixmap);
        if (!image) return 5;
        uint64_t hash = UINT64_C(14695981039346656037);
        for (int row = 0; row < h; ++row)
            for (int col = 0; col < w; ++col)
                hash = (hash ^ XGetPixel(image,col,row)) * UINT64_C(1099511628211);
        XDestroyImage(image);
        printf("%" PRIu64 " %" PRIu64 "\n", ns, hash); fflush(stdout);
    }
}
