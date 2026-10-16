(ns workload
  (:gen-class)
  (:require [core-workloads :as core])
  (:import [java.util.concurrent ArrayBlockingQueue]))

(defn handoff [n seed batch]
  (let [q (ArrayBlockingQueue. (int (quot 256 batch)))
        producer (future
                   (if (= batch 1)
                     (dotimes [i n] (.put q (+ i seed)))
                     (doseq [start (range 0 n batch)]
                       (.put q (mapv #(+ % seed) (range start (min n (+ start batch))))))))
        total (if (= batch 1)
                (loop [i 0 total 0]
                  (if (= i n) total (recur (inc i) (+ total (.take q)))))
                (reduce + (for [_ (range 0 n batch)] (reduce + (.take q)))))]
    @producer
    total))

(defn -main [& args]
  (let [[name n seed warmups repeats] args
        [n seed warmups repeats] (mapv parse-long [n seed warmups repeats])]
    (dotimes [iteration (+ warmups repeats)]
      (let [started (System/nanoTime)
            checksum (case name
                       "cpu" (core/cpu n seed)
                       "records" (core/records n seed)
                       "snapshots" (core/snapshots n seed)
                       "handoff" (handoff n seed 1)
                       "batches" (handoff n seed 64))
            elapsed (- (System/nanoTime) started)]
        (println "BENCH" iteration elapsed checksum)))
    (shutdown-agents)))
