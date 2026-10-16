/* Measurement only: Lua has no standard monotonic wall clock. */
#include <lua.h>
#include <time.h>

static int now(lua_State *state) {
    struct timespec ts;
    if (clock_gettime(CLOCK_MONOTONIC, &ts) != 0) {
        return 0;
    }
    lua_pushnumber(state, (double)ts.tv_sec * 1000000000.0 + (double)ts.tv_nsec);
    return 1;
}

int luaopen_benchclock(lua_State *state) {
    lua_pushcfunction(state, now);
    return 1;
}
