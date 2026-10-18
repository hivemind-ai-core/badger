// Instrumentation only: a clean launcher prevents Python's retained allocations
// becoming the exec child's RSS floor. No workload implementation lives here.
#define _DEFAULT_SOURCE
#include <errno.h>
#include <stdio.h>
#include <sys/resource.h>
#include <sys/wait.h>
#include <unistd.h>

int main(int argc, char **argv) {
    if (argc < 2) {
        return 125;
    }
    pid_t child = fork();
    if (child < 0) {
        perror("fork");
        return 125;
    }
    if (child == 0) {
        execvp(argv[1], argv + 1);
        perror("execvp");
        _exit(125);
    }
    int status;
    struct rusage usage;
    while (wait4(child, &status, 0, &usage) < 0) {
        if (errno != EINTR) {
            perror("wait4");
            return 125;
        }
    }
    fprintf(stderr, "[realworld child] %ld %ld %ld %ld %ld\n", usage.ru_maxrss,
            usage.ru_utime.tv_sec * 1000000L + usage.ru_utime.tv_usec,
            usage.ru_stime.tv_sec * 1000000L + usage.ru_stime.tv_usec,
            usage.ru_nvcsw, usage.ru_nivcsw);
    if (WIFEXITED(status)) {
        return WEXITSTATUS(status);
    }
    return 128 + WTERMSIG(status);
}
