import com.google.gson.Gson;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import java.util.concurrent.ArrayBlockingQueue;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;
import java.util.concurrent.Future;

public final class Workload {
    record Order(long id, long customer, long amount, boolean active, String note) {}

    static long score(long id, long seed, int rounds) {
        long x = (id + seed) % 1000003;
        for (int r = 0; r < rounds; r++) {
            x = (x * 17 + 23) % 1000003;
        }
        return x;
    }

    static long cpu(int n, long seed) {
        long total = 0;
        for (int id = 0; id < n; id++) {
            total += score(id, seed, 128);
        }
        return total;
    }

    static long reconcile(int n, long seed) {
        long[] discounts = new long[1024];
        for (int id = 0; id < discounts.length; id++) {
            discounts[id] = (id * 7L + seed) % 20;
        }
        var rows = new ArrayList<Order>(n);
        for (int id = 0; id < n; id++) {
            rows.add(new Order(id, id % 1024, (id * 31L + seed) % 100000, id % 7 != 0, "order"));
        }
        var enriched = rows.stream().filter(Order::active).map(o -> new Order(
            o.id(), o.customer(), o.amount() * (100 - discounts[(int) o.customer()]) / 100,
            o.active(), o.note())).toList();
        var groups = new HashMap<Long, Long>();
        for (var o : enriched) {
            groups.merge(o.customer(), o.amount(), Long::sum);
        }
        long total = groups.size();
        for (var entry : groups.entrySet()) {
            total += (entry.getKey() + 1) * entry.getValue();
        }
        return total;
    }

    static long jsonOrders(String input, Gson gson) {
        long total = 0;
        for (String line : input.split("\n")) {
            if (!line.isEmpty()) {
                Order o = gson.fromJson(line, Order.class);
                var enriched = new Order(o.id(), o.customer(), o.amount() + o.amount() / 5,
                                         o.active(), o.note());
                Order result = gson.fromJson(gson.toJson(enriched), Order.class);
                if (result.active()) {
                    total += (result.customer() + 1) * result.amount() + result.id()
                             + result.note().getBytes(StandardCharsets.UTF_8).length;
                }
            }
        }
        return total;
    }

    static long state(int n, long seed) {
        var current = new HashMap<Long, Long>();
        var versions = new ArrayList<Map<Long, Long>>();
        for (int id = 0; id < n; id++) {
            current.put(id % 1024L, (id * 31L + seed) % 100000);
            if ((id + 1) % 256 == 0) {
                versions.add(new HashMap<>(current));
            }
        }
        versions.add(new HashMap<>(current));
        long total = 0;
        for (var version : versions) {
            for (var entry : version.entrySet()) {
                total += (entry.getKey() + 1) * entry.getValue();
            }
        }
        return total;
    }

    static final class Pipeline implements AutoCloseable {
        final int workers;
        final ExecutorService pool;

        Pipeline(int workers) {
            this.workers = workers;
            // N consumers and one producer. Threads start on first submission and
            // persist across warm iterations; final shutdown is outside kernel time.
            this.pool = Executors.newFixedThreadPool(workers + 1);
        }

        long run(int n, long seed, int rounds) throws Exception {
            var jobs = new ArrayBlockingQueue<long[]>(4);
            var results = new ArrayBlockingQueue<Long>(4);
            var tasks = new ArrayList<Future<?>>();
            tasks.add(pool.submit(() -> {
                for (int start = 0; start < n; start += 64) {
                    long[] batch = new long[Math.min(64, n - start)];
                    for (int i = 0; i < batch.length; i++) {
                        batch[i] = start + i;
                    }
                    jobs.put(batch);
                }
                // Standard poison-pill shutdown; no data ID is used as a sentinel.
                for (int w = 0; w < workers; w++) {
                    jobs.put(new long[0]);
                }
                return null;
            }));
            for (int w = 0; w < workers; w++) {
                tasks.add(pool.submit(() -> {
                    while (true) {
                        long[] batch = jobs.take();
                        if (batch.length == 0) {
                            break;
                        }
                        long total = 0;
                        for (long id : batch) {
                            total += score(id, seed, rounds);
                        }
                        results.put(total);
                    }
                    return null;
                }));
            }
            long total = 0;
            for (int b = 0; b < (n + 63) / 64; b++) {
                total += results.take();
            }
            for (var task : tasks) {
                task.get();
            }
            return total;
        }

        @Override
        public void close() {
            pool.close();
        }
    }

    public static void main(String[] args) throws Exception {
        String name = args[0];
        int n = Integer.parseInt(args[1]);
        long seed = Long.parseLong(args[2]);
        int workers = Integer.parseInt(args[3]);
        int warmups = Integer.parseInt(args[4]);
        int repeats = Integer.parseInt(args[5]);
        System.out.println("READY");
        System.out.flush();
        // Fixture reads are outside kernel time, inside process time and RSS.
        String input = name.equals("json") ? Files.readString(Path.of(args[6]), StandardCharsets.UTF_8) : "";
        Gson gson = name.equals("json") ? new Gson() : null;
        Pipeline pipeline = null;
        try {
            for (int iteration = 0; iteration < warmups + repeats; iteration++) {
                long start = System.nanoTime();
                long checksum;
                if (name.equals("messaging") || name.equals("pipeline")) {
                    if (pipeline == null) {
                        pipeline = new Pipeline(workers);
                    }
                    checksum = pipeline.run(n, seed, name.equals("messaging") ? 2 : 128);
                } else {
                    checksum = switch (name) {
                        case "cpu" -> cpu(n, seed);
                        case "reconcile" -> reconcile(n, seed);
                        case "json" -> jsonOrders(input, gson);
                        case "state" -> state(n, seed);
                        default -> throw new IllegalArgumentException("unknown workload: " + name);
                    };
                }
                long elapsed = System.nanoTime() - start;
                System.out.printf("BENCH %d %d %d%n", iteration, elapsed, checksum);
            }
        } finally {
            if (pipeline != null) {
                pipeline.close();
            }
        }
    }
}
