import java.util.*;
import java.util.concurrent.*;

public class Workload {
    record Event(long id, long value, boolean active) {}

    static long cpu(int n, long seed) {
        long total = 0;
        for (int i = 0; i < n; i++) {
            long x = (i + seed) % 1000003;
            for (int r = 0; r < 8; r++) { x = (x * 17 + 23) % 1000003; }
            total += x;
        }
        return total;
    }

    static long records(int n, long seed) {
        var original = new ArrayList<Event>(n);
        for (int i = 0; i < n; i++) { original.add(new Event(i, (i + seed) % 1000, i % 3 != 0)); }
        var enriched = original.stream().map(e -> new Event(e.id(), e.value() * 3 + 7, e.active())).toList();
        return enriched.stream().filter(Event::active).mapToLong(Event::value).sum()
            + original.stream().mapToLong(Event::value).sum();
    }

    static long snapshots(int n, long seed) {
        var current = new HashMap<Integer, Long>();
        var versions = new ArrayList<Map<Integer, Long>>();
        for (int i = 0; i < n; i++) {
            current.put(i % 256, i + seed);
            if ((i + 1) % 64 == 0) { versions.add(new HashMap<>(current)); }
        }
        versions.add(new HashMap<>(current));
        long total = 0;
        for (var version : versions) {
            for (var entry : version.entrySet()) { total += (entry.getKey() + 1L) * entry.getValue(); }
        }
        return total;
    }

    static long handoff(int n, long seed, int batch) throws Exception {
        // One virtual producer, main thread consumer; no executor warm pool.
        long total = 0;
        if (batch == 1) {
            var q = new ArrayBlockingQueue<Long>(256);
            var producer = Thread.ofVirtual().start(() -> {
                try { for (int i = 0; i < n; i++) { q.put(i + seed); } }
                catch (InterruptedException e) { throw new RuntimeException(e); }
            });
            for (int i = 0; i < n; i++) { total += q.take(); }
            producer.join();
        } else {
            var q = new ArrayBlockingQueue<List<Long>>(4);
            var producer = Thread.ofVirtual().start(() -> {
                try {
                    for (int start = 0; start < n; start += 64) {
                        var values = new ArrayList<Long>(64);
                        for (int i = start; i < Math.min(start + 64, n); i++) { values.add(i + seed); }
                        q.put(values);
                    }
                } catch (InterruptedException e) { throw new RuntimeException(e); }
            });
            for (int start = 0; start < n; start += 64) {
                for (long x : q.take()) { total += x; }
            }
            producer.join();
        }
        return total;
    }

    public static void main(String[] args) throws Exception {
        String name = args[0];
        int n = Integer.parseInt(args[1]), warmups = Integer.parseInt(args[3]), repeats = Integer.parseInt(args[4]);
        long seed = Long.parseLong(args[2]);
        for (int iteration = 0; iteration < warmups + repeats; iteration++) {
            long started = System.nanoTime();
            long checksum = switch (name) {
                case "cpu" -> cpu(n, seed);
                case "records" -> records(n, seed);
                case "snapshots" -> snapshots(n, seed);
                case "handoff" -> handoff(n, seed, 1);
                case "batches" -> handoff(n, seed, 64);
                default -> throw new IllegalArgumentException(name);
            };
            long elapsed = System.nanoTime() - started;
            System.out.println("BENCH " + iteration + " " + elapsed + " " + checksum);
        }
    }
}
