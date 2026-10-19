// LD_PRELOAD probe for timing YouTFast's startup without changing the binary
// (see docs/benchmarks.md). Every event is one line
// "<CLOCK_MONOTONIC seconds> <thread id> <event...>" appended to $YTPROBE_LOG:
// frames (eglSwapBuffers), texture uploads (glTexImage2D: icons, thumbnails),
// files opened, connections, DNS lookups, threads, dlopen and mpv's start.
// Only timings, sizes, counts, addresses and file paths are recorded.
//
//   cc -O2 -fPIC -shared -o probe.so probe.c -ldl
#define _GNU_SOURCE
#include <arpa/inet.h>
#include <dlfcn.h>
#include <fcntl.h>
#include <netdb.h>
#include <netinet/in.h>
#include <pthread.h>
#include <stdarg.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/socket.h>
#include <sys/syscall.h>
#include <sys/un.h>
#include <time.h>
#include <unistd.h>

static int log_fd = -1;
static void *(*real_dlsym)(void *, const char *);

static double now(void) {
    struct timespec ts;
    clock_gettime(CLOCK_MONOTONIC, &ts);
    return (double)ts.tv_sec + (double)ts.tv_nsec / 1e9;
}

static void emit(const char *fmt, ...) {
    if (log_fd < 0) return;
    char line[1024];
    int n = snprintf(line, sizeof line, "%.6f %ld ", now(), (long)syscall(SYS_gettid));
    va_list ap;
    va_start(ap, fmt);
    n += vsnprintf(line + n, sizeof line - n - 1, fmt, ap);
    va_end(ap);
    if (n > (int)sizeof line - 2) n = sizeof line - 2;
    line[n++] = '\n';
    ssize_t ignored = write(log_fd, line, n);
    (void)ignored;
}

static void *next(const char *name) {
    if (!real_dlsym) {
        real_dlsym = dlvsym(RTLD_NEXT, "dlsym", "GLIBC_2.34");
        if (!real_dlsym) real_dlsym = dlvsym(RTLD_NEXT, "dlsym", "GLIBC_2.2.5");
    }
    return real_dlsym(RTLD_NEXT, name);
}

__attribute__((constructor)) static void probe_init(void) {
    const char *path = getenv("YTPROBE_LOG");
    if (path) log_fd = (int)syscall(SYS_openat, AT_FDCWD, path, O_WRONLY | O_CREAT | O_APPEND | O_CLOEXEC, 0600);
    emit("ctor pid=%d", (int)getpid());
}

// ---- files -----------------------------------------------------------------
static int (*real_open)(const char *, int, ...);
static int (*real_open64)(const char *, int, ...);
static int (*real_openat)(int, const char *, int, ...);
static int (*real_openat64)(int, const char *, int, ...);

static void log_open(const char *what, const char *path, int flags, int fd, double t0) {
    emit("open %s fd=%d flags=%#x dt=%.6f %s", what, fd, flags, now() - t0, path ? path : "?");
}

int open(const char *path, int flags, ...) {
    if (!real_open) real_open = next("open");
    mode_t mode = 0;
    if (flags & (O_CREAT | O_TMPFILE)) { va_list ap; va_start(ap, flags); mode = va_arg(ap, mode_t); va_end(ap); }
    double t0 = now();
    int fd = real_open(path, flags, mode);
    log_open("open", path, flags, fd, t0);
    return fd;
}
int open64(const char *path, int flags, ...) {
    if (!real_open64) real_open64 = next("open64");
    mode_t mode = 0;
    if (flags & (O_CREAT | O_TMPFILE)) { va_list ap; va_start(ap, flags); mode = va_arg(ap, mode_t); va_end(ap); }
    double t0 = now();
    int fd = real_open64(path, flags, mode);
    log_open("open64", path, flags, fd, t0);
    return fd;
}
int openat(int dir, const char *path, int flags, ...) {
    if (!real_openat) real_openat = next("openat");
    mode_t mode = 0;
    if (flags & (O_CREAT | O_TMPFILE)) { va_list ap; va_start(ap, flags); mode = va_arg(ap, mode_t); va_end(ap); }
    double t0 = now();
    int fd = real_openat(dir, path, flags, mode);
    log_open("openat", path, flags, fd, t0);
    return fd;
}
int openat64(int dir, const char *path, int flags, ...) {
    if (!real_openat64) real_openat64 = next("openat64");
    mode_t mode = 0;
    if (flags & (O_CREAT | O_TMPFILE)) { va_list ap; va_start(ap, flags); mode = va_arg(ap, mode_t); va_end(ap); }
    double t0 = now();
    int fd = real_openat64(dir, path, flags, mode);
    log_open("openat64", path, flags, fd, t0);
    return fd;
}

int rename(const char *from, const char *to) {
    static int (*real)(const char *, const char *);
    if (!real) real = next("rename");
    int r = real(from, to);
    emit("rename r=%d %s", r, to ? to : "?");
    return r;
}

// ---- network -----------------------------------------------------------------
int connect(int fd, const struct sockaddr *addr, socklen_t len) {
    static int (*real)(int, const struct sockaddr *, socklen_t);
    if (!real) real = next("connect");
    double t0 = now();
    int r = real(fd, addr, len);
    char host[INET6_ADDRSTRLEN + 8] = "?";
    int port = 0;
    if (addr && addr->sa_family == AF_INET) {
        const struct sockaddr_in *in = (const void *)addr;
        inet_ntop(AF_INET, &in->sin_addr, host, sizeof host);
        port = ntohs(in->sin_port);
    } else if (addr && addr->sa_family == AF_INET6) {
        const struct sockaddr_in6 *in = (const void *)addr;
        inet_ntop(AF_INET6, &in->sin6_addr, host, sizeof host);
        port = ntohs(in->sin6_port);
    } else if (addr && addr->sa_family == AF_UNIX) {
        const struct sockaddr_un *un = (const void *)addr;
        snprintf(host, sizeof host, "%.40s", un->sun_path[0] ? un->sun_path : "@abstract");
    }
    emit("connect fd=%d r=%d dt=%.6f %s %d", fd, r, now() - t0, host, port);
    return r;
}

int getaddrinfo(const char *node, const char *service, const struct addrinfo *hints, struct addrinfo **res) {
    static int (*real)(const char *, const char *, const struct addrinfo *, struct addrinfo **);
    if (!real) real = next("getaddrinfo");
    double t0 = now();
    int r = real(node, service, hints, res);
    emit("dns r=%d dt=%.6f %s", r, now() - t0, node ? node : "?");
    return r;
}

// ---- threads, libraries, mpv ------------------------------------------------------
int pthread_create(pthread_t *thread, const pthread_attr_t *attr, void *(*start)(void *), void *arg) {
    static int (*real)(pthread_t *, const pthread_attr_t *, void *(*)(void *), void *);
    if (!real) real = next("pthread_create");
    int r = real(thread, attr, start, arg);
    emit("thread r=%d", r);
    return r;
}

void *dlopen(const char *file, int mode) {
    static void *(*real)(const char *, int);
    if (!real) real = next("dlopen");
    double t0 = now();
    void *h = real(file, mode);
    emit("dlopen ok=%d dt=%.6f %s", h != NULL, now() - t0, file ? file : "(self)");
    return h;
}

#define MPV_TIMED(name, ret, params, args)                       \
    ret name params {                                            \
        static ret (*real) params;                               \
        if (!real) real = next(#name);                           \
        double t0 = now();                                       \
        ret r = real args;                                       \
        emit("%s dt=%.6f", #name, now() - t0);                   \
        return r;                                                \
    }
MPV_TIMED(mpv_create, void *, (void), ())
MPV_TIMED(mpv_initialize, int, (void *ctx), (ctx))
MPV_TIMED(mpv_render_context_create, int, (void **res, void *mpv, void *params), (res, mpv, params))

// ---- EGL/GL: frames and texture uploads ----------------------------------------
typedef unsigned int (*swap_fn)(void *, void *);
typedef unsigned int (*swap_damage_fn)(void *, void *, int *, int);
typedef void *(*gpa_fn)(const char *);
typedef void (*teximage_fn)(unsigned, int, int, int, int, int, unsigned, unsigned, const void *);
typedef void (*texsub_fn)(unsigned, int, int, int, int, int, unsigned, unsigned, const void *);

static swap_fn real_swap;
static swap_damage_fn real_swap_damage_khr, real_swap_damage_ext;
static gpa_fn real_gpa;
static teximage_fn real_teximage;
static texsub_fn real_texsub;
static unsigned long frames;

static unsigned int my_swap(void *dpy, void *surface) {
    double t0 = now();
    unsigned int r = real_swap(dpy, surface);
    emit("swap n=%lu dt=%.6f", ++frames, now() - t0);
    return r;
}
static unsigned int my_swap_damage_khr(void *dpy, void *surface, int *rects, int n) {
    double t0 = now();
    unsigned int r = real_swap_damage_khr(dpy, surface, rects, n);
    emit("swap n=%lu dt=%.6f damage", ++frames, now() - t0);
    return r;
}
static unsigned int my_swap_damage_ext(void *dpy, void *surface, int *rects, int n) {
    double t0 = now();
    unsigned int r = real_swap_damage_ext(dpy, surface, rects, n);
    emit("swap n=%lu dt=%.6f damage", ++frames, now() - t0);
    return r;
}
static void my_teximage(unsigned target, int level, int internal, int w, int h, int border, unsigned format,
                        unsigned type, const void *data) {
    double t0 = now();
    real_teximage(target, level, internal, w, h, border, format, type, data);
    emit("tex %dx%d data=%d dt=%.6f", w, h, data != NULL, now() - t0);
}
static void my_texsub(unsigned target, int level, int x, int y, int w, int h, unsigned format, unsigned type,
                      const void *data) {
    real_texsub(target, level, x, y, w, h, format, type, data);
    emit("texsub %dx%d", w, h);
}

static void *wrap(const char *symbol, void *real) {
    if (!real || !symbol) return real;
    if (!strcmp(symbol, "eglSwapBuffers")) { real_swap = real; return (void *)my_swap; }
    if (!strcmp(symbol, "eglSwapBuffersWithDamageKHR")) { real_swap_damage_khr = real; return (void *)my_swap_damage_khr; }
    if (!strcmp(symbol, "eglSwapBuffersWithDamageEXT")) { real_swap_damage_ext = real; return (void *)my_swap_damage_ext; }
    if (!strcmp(symbol, "glTexImage2D")) { real_teximage = real; return (void *)my_teximage; }
    if (!strcmp(symbol, "glTexSubImage2D")) { real_texsub = real; return (void *)my_texsub; }
    return real;
}

static void *my_gpa(const char *symbol) {
    return wrap(symbol, real_gpa(symbol));
}

void *dlsym(void *handle, const char *symbol) {
    if (!real_dlsym) {
        real_dlsym = dlvsym(RTLD_NEXT, "dlsym", "GLIBC_2.34");
        if (!real_dlsym) real_dlsym = dlvsym(RTLD_NEXT, "dlsym", "GLIBC_2.2.5");
    }
    void *found = real_dlsym(handle, symbol);
    if (found && !strcmp(symbol, "eglGetProcAddress")) {
        real_gpa = found;
        return (void *)my_gpa;
    }
    return wrap(symbol, found);
}

// Programs that link EGL directly reach these through the PLT instead of dlsym.
void *eglGetProcAddress(const char *symbol) {
    if (!real_gpa) real_gpa = next("eglGetProcAddress");
    return real_gpa ? wrap(symbol, real_gpa(symbol)) : NULL;
}
