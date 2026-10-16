(ns core-workloads)

(defn cpu [n seed]
  (reduce + (map (fn [i]
                   (loop [r 0 x (mod (+ i seed) 1000003)]
                     (if (= r 8) x (recur (inc r) (mod (+ (* x 17) 23) 1000003)))))
                 (range n))))

(defn records [n seed]
  (let [original (mapv (fn [i] {:id i :value (mod (+ i seed) 1000) :active (not (zero? (mod i 3)))}) (range n))
        enriched (mapv #(update % :value (fn [v] (+ (* v 3) 7))) original)]
    (+ (transduce (comp (filter :active) (map :value)) + enriched)
       (transduce (map :value) + original))))

(defn snapshots [n seed]
  (let [[current versions]
        (reduce (fn [[current versions] i]
                  (let [updated (assoc current (mod i 256) (+ i seed))]
                    [updated (if (zero? (mod (inc i) 64)) (conj versions updated) versions)]))
                [{} []] (range n))]
    (reduce + (for [version (conj versions current) [k v] version] (* (inc k) v)))))
