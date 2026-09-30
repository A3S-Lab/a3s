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

static int is_delegated_cgroup_path(const char *path) {
    /* Accept the delegated subtree for any user slice: a single-identity
     * root deployment passes the sandbox account's delegated root rather
     * than the caller's own. The shape is still exact — no traversal, no
     * extra components beyond user-<N>.slice/user@<N>.service/a3s-box-delegated. */
    static const char *const prefix = "/sys/fs/cgroup/user.slice/user-";
    static const char *const middle = ".slice/user@";
    static const char *const suffix = ".service/a3s-box-delegated";
    size_t len = strlen(path);
    size_t at = strlen(prefix);
    if (len <= at + strlen(middle) + strlen(suffix) ||
        strncmp(path, prefix, at) != 0) {
        return 0;
    }
    for (size_t i = at; i < len - strlen(suffix) - strlen(middle); i++) {
        if (path[i] < '0' || path[i] > '9') {
            return 0;
        }
        if (strncmp(path + i, middle, strlen(middle)) == 0) {
            at = i + strlen(middle);
            for (size_t j = at; j < len - strlen(suffix); j++) {
                if (strncmp(path + j, suffix, strlen(suffix)) == 0) {
                    return j > at;
                }
                if (path[j] < '0' || path[j] > '9') {
                    return 0;
                }
            }
            return 0;
        }
    }
    return 0;
}

static int format_env(char *buffer, size_t buffer_len, const char *key, const char *value) {
    int written = snprintf(buffer, buffer_len, "%s=%s", key, value);
    return written < 0 || (size_t)written >= buffer_len ? -1 : 0;
}

int main(int argc, char **argv, char **envp) {
    (void)envp;
    uid_t uid = getuid();
    char home_env[512];
    char user_env[256];
    char logname_env[256];
    struct passwd *pw;

    /* A root caller is allowed for single-identity service deployments: the
     * agent itself runs as root, and the state-ownership contract of the
     * runtime requires one effective identity across boot, removal, and log
     * shipping. The setuid-root installation check stays. */
    if (geteuid() != 0) {
        return fail("requires a root-owned setuid installation");
    }
    if (argc != 11 || strcmp(argv[1], "native-linux-service") != 0 ||
        strcmp(argv[2], "--root") != 0 || argv[3][0] != '/' ||
        strcmp(argv[4], "--agent") != 0 || argv[5][0] != '/' ||
        strcmp(argv[6], "--delegated-cgroup-root") != 0 ||
        !is_delegated_cgroup_path(argv[7]) ||
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
