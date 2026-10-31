// QA helper: inspect or temporarily select an enabled macOS input source.
#include <Carbon/Carbon.h>
#include <stdio.h>
#include <string.h>
int main(int argc, char **argv) {
    if (argc == 1) {
        TISInputSourceRef source = TISCopyCurrentKeyboardInputSource();
        CFStringRef id = TISGetInputSourceProperty(source, kTISPropertyInputSourceID);
        char name[512];
        Boolean ok = CFStringGetCString(id, name, sizeof(name), kCFStringEncodingUTF8);
        if (ok) puts(name);
        CFRelease(source);
        return ok ? 0 : 1;
    }
    if (argc != 2) return 2;
    CFArrayRef sources = TISCreateInputSourceList(NULL, false);
    int result = 1;
    for (CFIndex i = 0; i < CFArrayGetCount(sources); ++i) {
        TISInputSourceRef source = (TISInputSourceRef)CFArrayGetValueAtIndex(sources, i);
        CFStringRef id = TISGetInputSourceProperty(source, kTISPropertyInputSourceID);
        char name[512];
        if (CFStringGetCString(id, name, sizeof(name), kCFStringEncodingUTF8) && strcmp(name, argv[1]) == 0) {
            result = TISSelectInputSource(source) == noErr ? 0 : 1;
            break;
        }
    }
    CFRelease(sources);
    return result;
}
