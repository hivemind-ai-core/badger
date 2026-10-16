#include <algorithm>
#include <chrono>
#include <condition_variable>
#include <cstdint>
#include <deque>
#include <iostream>
#include <mutex>
#include <stdexcept>
#include <string>
#include <thread>
#include <unordered_map>
#include <vector>

using Int = std::int64_t;
struct Event { Int id, value; bool active; };

Int cpu(Int n, Int seed) {
    Int total = 0;
    for (Int i = 0; i < n; ++i) {
        Int x = (i + seed) % 1000003;
        for (int r = 0; r < 8; ++r) { x = (x * 17 + 23) % 1000003; }
        total += x;
    }
    return total;
}

Int records(Int n, Int seed) {
    std::vector<Event> original, enriched;
    original.reserve(n); enriched.reserve(n);
    for (Int i = 0; i < n; ++i) { original.push_back({i, (i + seed) % 1000, i % 3 != 0}); }
    std::transform(original.begin(), original.end(), std::back_inserter(enriched), [](Event e) { e.value = e.value * 3 + 7; return e; });
    Int total = 0;
    for (const auto& e : enriched) { if (e.active) { total += e.value; } }
    for (const auto& e : original) { total += e.value; }
    return total;
}

Int snapshots(Int n, Int seed) {
    std::unordered_map<Int, Int> current;
    std::vector<std::unordered_map<Int, Int>> versions;
    for (Int i = 0; i < n; ++i) {
        current[i % 256] = i + seed;
        if ((i + 1) % 64 == 0) { versions.push_back(current); }
    }
    versions.push_back(current);
    Int total = 0;
    for (const auto& version : versions) {
        for (const auto& [k, v] : version) { total += (k + 1) * v; }
    }
    return total;
}

// C++ has no standard bounded channel; conventional mutex/CV queue, no spin loop.
template <typename T> class Queue {
    std::mutex mutex;
    std::condition_variable readable, writable;
    std::deque<T> values;
    std::size_t capacity;
public:
    explicit Queue(std::size_t size) : capacity(size) {}
    void put(T value) {
        std::unique_lock lock(mutex);
        writable.wait(lock, [&] { return values.size() < capacity; });
        values.push_back(std::move(value));
        readable.notify_one();
    }
    T take() {
        std::unique_lock lock(mutex);
        readable.wait(lock, [&] { return !values.empty(); });
        T value = std::move(values.front()); values.pop_front();
        writable.notify_one();
        return value;
    }
};

Int handoff(Int n, Int seed, Int batch) {
    Int total = 0;
    if (batch == 1) {
        Queue<Int> q(256);
        std::jthread producer([&] { for (Int i = 0; i < n; ++i) { q.put(i + seed); } });
        for (Int i = 0; i < n; ++i) { total += q.take(); }
    } else {
        Queue<std::vector<Int>> q(4);
        std::jthread producer([&] {
            for (Int start = 0; start < n; start += 64) {
                std::vector<Int> values; values.reserve(64);
                for (Int i = start; i < std::min(start + 64, n); ++i) { values.push_back(i + seed); }
                q.put(std::move(values));
            }
        });
        for (Int start = 0; start < n; start += 64) {
            for (Int x : q.take()) { total += x; }
        }
    }
    return total;
}

int main(int argc, char** argv) {
    if (argc != 6) { return 2; }
    const std::string name(argv[1]);
    const Int n = std::stoll(argv[2]), seed = std::stoll(argv[3]);
    const Int warmups = std::stoll(argv[4]), repeats = std::stoll(argv[5]);
    for (Int iteration = 0; iteration < warmups + repeats; ++iteration) {
        const auto started = std::chrono::steady_clock::now();
        Int checksum;
        if (name == "cpu") { checksum = cpu(n, seed); }
        else if (name == "records") { checksum = records(n, seed); }
        else if (name == "snapshots") { checksum = snapshots(n, seed); }
        else if (name == "handoff") { checksum = handoff(n, seed, 1); }
        else if (name == "batches") { checksum = handoff(n, seed, 64); }
        else { throw std::invalid_argument(name); }
        const auto elapsed = std::chrono::duration_cast<std::chrono::nanoseconds>(std::chrono::steady_clock::now() - started).count();
        std::cout << "BENCH " << iteration << ' ' << elapsed << ' ' << checksum << '\n';
    }
}
