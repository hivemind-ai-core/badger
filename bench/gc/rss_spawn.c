// A small clean process measures the language child's RSS after fork/exec.
// This avoids Python's report/hash allocations becoming the child's RSS floor.
#define _DEFAULT_SOURCE
#include <errno.h>
#include <stdio.h>
#include <sys/resource.h>
#include <sys/wait.h>
#include <unistd.h>

int main(int argc, char **argv) {
    if (argc < 2) { return 125; }
    pid_t pid = fork();
    if (pid < 0) { perror("fork"); return 125; }
    if (pid == 0) {
        execvp(argv[1], argv + 1);
        perror("execvp");
        _exit(125);
    }
    int status;
    struct rusage usage;
    while (wait4(pid, &status, 0, &usage) < 0) {
        if (errno != EINTR) { perror("wait4"); return 125; }
    }
    fprintf(stderr, "[gc-bench child rss] %ld\n", usage.ru_maxrss);
    return WIFEXITED(status) ? WEXITSTATUS(status) : 128 + WTERMSIG(status);
}
