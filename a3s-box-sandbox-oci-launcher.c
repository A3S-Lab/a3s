#define _GNU_SOURCE
#include <errno.h>
#include <fcntl.h>
#include <grp.h>
#include <stdio.h>
#include <string.h>
#include <sys/stat.h>
#include <unistd.h>

static const char *const RUNTIME_DIR = "/usr/local/libexec/a3s-box-sandbox";
static const char *const DELEGATION =
    "/sys/fs/cgroup/user.slice/user-1000.slice/user@1000.service/a3s-box-delegated";

static int fail(const char *message) {
    fprintf(stderr, "sandbox OCI launcher: %s\n", message);
    return 1;
}

int main(int argc, char **argv, char **envp) {
    (void)envp;
    if (argc != 11 || strcmp(argv[1], "native-linux-service") != 0 ||
        strcmp(argv[2], "--root") != 0 || argv[3][0] != '/' ||
        strcmp(argv[4], "--agent") != 0 || argv[5][0] != '/' ||
        strcmp(argv[6], "--delegated-cgroup-root") != 0 ||
        strcmp(argv[7], DELEGATION) != 0 ||
        strcmp(argv[8], "--container-id") != 0 || argv[9][0] == '\0' ||
        argv[9][0] == '-' || strcmp(argv[10], "--a3s-box-control-fds") != 0) {
        return fail("only the configured Sandbox native-linux-service invocation is allowed");
    }
    if (getuid() != 1000 || getgid() != 1000 || geteuid() != 0) {
        return fail("requires caller 1000:1000 and a root-owned setuid installation");
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
    char *const runtime_env[] = {
        "HOME=/home/roylin", "USER=roylin", "LOGNAME=roylin",
        "PATH=/usr/sbin:/usr/bin:/sbin:/bin", "LC_ALL=C", NULL
    };
    if (chdir("/") != 0) {
        return fail("could not set a trusted working directory");
    }
    fexecve(runtime, argv, runtime_env);
    return fail(strerror(errno));
}
