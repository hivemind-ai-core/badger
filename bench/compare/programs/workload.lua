-- Portable Lua / LuaJIT code. Lua tables, explicit table snapshots.
-- A tiny C measurement module supplies CLOCK_MONOTONIC, never workload code.
local now = require('benchclock')
local function cpu(n, seed)
    local total = 0
    for i = 0, n - 1 do
        local x = (i + seed) % 1000003
        for _ = 1, 8 do
            x = (x * 17 + 23) % 1000003
        end
        total = total + x
    end
    return total
end

local function records(n, seed)
    local original, enriched = {}, {}
    for i = 0, n - 1 do
        original[#original + 1] = { id = i, value = (i + seed) % 1000, active = i % 3 ~= 0 }
    end
    for _, e in ipairs(original) do
        enriched[#enriched + 1] = { id = e.id, value = e.value * 3 + 7, active = e.active }
    end
    local total = 0
    for _, e in ipairs(enriched) do
        if e.active then
            total = total + e.value
        end
    end
    for _, e in ipairs(original) do
        total = total + e.value
    end
    return total
end

local function clone(current)
    local copy = {}
    for k, v in pairs(current) do
        copy[k] = v
    end
    return copy
end

local function snapshots(n, seed)
    local current, versions = {}, {}
    for i = 0, n - 1 do
        current[i % 256] = i + seed
        if (i + 1) % 64 == 0 then
            versions[#versions + 1] = clone(current)
        end
    end
    versions[#versions + 1] = clone(current)
    local total = 0
    for _, version in ipairs(versions) do
        for k, v in pairs(version) do
            total = total + (k + 1) * v
        end
    end
    return total
end

local name = arg[1]
local n, seed, warmups, repeats = tonumber(arg[2]), tonumber(arg[3]), tonumber(arg[4]), tonumber(arg[5])
local workloads = { cpu = cpu, records = records, snapshots = snapshots }
for iteration = 0, warmups + repeats - 1 do
    local started = now()
    local checksum = workloads[name](n, seed)
    local elapsed = now() - started
    print(string.format("BENCH %d %.0f %.0f", iteration, elapsed, checksum))
end
