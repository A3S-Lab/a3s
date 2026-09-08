#define _GNU_SOURCE
#include <errno.h>
#include <fcntl.h>
#include <grp.h>
#include <pwd.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/stat.h>
#include <unistd.h>

static const char *const RUNTIME_DIR = "/usr/local/libexec/a3s-box-sandbox";

static int fail(const char *message) {
    fprintf(stderr, "sandbox OCI launcher: %s\n", message);
    return 1;
}

static int build_delegation_root(char *buffer, size_t buffer_len, uid_t uid) {
    int written = snprintf(
        buffer,
        buffer_len,
        "/sys/fs/cgroup/user.slice/user-%u.slice/user@%u.service/a3s-box-delegated",
        (unsigned)uid,
        (unsigned)uid);
    return written < 0 || (size_t)written >= buffer_len ? -1 : 0;
}

static int format_env(char *buffer, size_t buffer_len, const char *key, const char *value) {
    int written = snprintf(buffer, buffer_len, "%s=%s", key, value);
    return written < 0 || (size_t)written >= buffer_len ? -1 : 0;
}

int main(int argc, char **argv, char **envp) {
    (void)envp;
    uid_t uid = getuid();
    gid_t gid = getgid();
    char delegation[256];
    char home_env[512];
    char user_env[256];
    char logname_env[256];
    struct passwd *pw;

    if (uid == 0 || gid == 0 || geteuid() != 0) {
        return fail("requires an unprivileged caller and a root-owned setuid installation");
    }
    if (build_delegation_root(delegation, sizeof(delegation), uid) != 0) {
        return fail("could not build delegated cgroup path");
    }
    if (argc != 11 || strcmp(argv[1], "native-linux-service") != 0 ||
        strcmp(argv[2], "--root") != 0 || argv[3][0] != '/' ||
        strcmp(argv[4], "--agent") != 0 || argv[5][0] != '/' ||
        strcmp(argv[6], "--delegated-cgroup-root") != 0 ||
        strcmp(argv[7], delegation) != 0 ||
        strcmp(argv[8], "--container-id") != 0 || argv[9][0] == '\0' ||
        argv[9][0] == '-' || strcmp(argv[10], "--a3s-box-control-fds") != 0) {
        return fail("only the configured Sandbox native-linux-service invocation is allowed");
    }
    int runtime_dir = open(RUNTIME_DIR, O_RDONLY | O_DIRECTORY | O_NOFOLLOW | O_CLOEXEC);
    struct stat metadata;
    if (runtime_dir < 0 || fstat(runtime_dir, &metadata) != 0 ||
        metadata.st_uid != 0 || metadata.st_gid != 0 || (metadata.st_mode & 0022) != 0) {
        return fail("runtime directory must be owned and writable only by root");
    }
    int runtime = openat(runtime_dir, "a3s-oci", O_RDONLY | O_NOFOLLOW | O_CLOEXEC);
    close(runtime_dir);
    if (runtime < 0 || fstat(runtime, &metadata) != 0 || !S_ISREG(metadata.st_mode) ||
        metadata.st_uid != 0 || metadata.st_gid != 0 || (metadata.st_mode & 0022) != 0) {
        return fail("runtime must be a regular executable owned and writable only by root");
    }
    if (setegid(0) != 0) {
        return fail("could not acquire effective root GID");
    }
    if (setgroups(0, NULL) != 0) {
        return errno == 0 ? 1 : errno;
    }
    pw = getpwuid(uid);
    if (pw == NULL || pw->pw_dir == NULL || pw->pw_dir[0] != '/' ||
        pw->pw_name == NULL || pw->pw_name[0] == '\0') {
        return fail("could not resolve the calling user identity");
    }
    if (format_env(home_env, sizeof(home_env), "HOME", pw->pw_dir) != 0 ||
        format_env(user_env, sizeof(user_env), "USER", pw->pw_name) != 0 ||
        format_env(logname_env, sizeof(logname_env), "LOGNAME", pw->pw_name) != 0) {
        return fail("could not materialize a trusted user environment");
    }
    char *const runtime_env[] = {
        home_env, user_env, logname_env, "PATH=/usr/sbin:/usr/bin:/sbin:/bin", "LC_ALL=C", NULL
    };
    if (chdir("/") != 0) {
        return fail("could not set a trusted working directory");
    }
    fexecve(runtime, argv, runtime_env);
    return fail(strerror(errno));
}
