package main

import (
	"bufio"
	"encoding/json"
	"fmt"
	"os"
	"strconv"
	"strings"
	"sync"
	"time"
)

type Order struct {
	ID       int64  `json:"id"`
	Customer int64  `json:"customer"`
	Amount   int64  `json:"amount"`
	Active   bool   `json:"active"`
	Note     string `json:"note"`
}

func score(id, seed, rounds int64) int64 {
	x := (id + seed) % 1000003
	for r := int64(0); r < rounds; r++ {
		x = (x*17 + 23) % 1000003
	}
	return x
}

func cpu(n, seed int64) int64 {
	var total int64
	for id := int64(0); id < n; id++ {
		total += score(id, seed, 128)
	}
	return total
}

func reconcile(n, seed int64) int64 {
	discounts := make([]int64, 1024)
	for id := range discounts {
		discounts[id] = (int64(id)*7 + seed) % 20
	}
	rows := make([]Order, n)
	for id := range rows {
		i := int64(id)
		rows[id] = Order{i, i % 1024, (i*31 + seed) % 100000, i%7 != 0, "order"}
	}
	enriched := make([]Order, 0, n)
	for _, o := range rows {
		if o.Active {
			o.Amount = o.Amount * (100 - discounts[o.Customer]) / 100
			enriched = append(enriched, o)
		}
	}
	groups := make(map[int64]int64)
	for _, o := range enriched {
		groups[o.Customer] += o.Amount
	}
	total := int64(len(groups))
	for customer, amount := range groups {
		total += (customer + 1) * amount
	}
	return total
}

func jsonOrders(input string) int64 {
	var total int64
	for _, line := range strings.Split(input, "\n") {
		if line != "" {
			var o Order
			if err := json.Unmarshal([]byte(line), &o); err != nil {
				panic(err)
			}
			o.Amount += o.Amount / 5
			encoded, err := json.Marshal(o)
			if err != nil {
				panic(err)
			}
			var result Order
			if err := json.Unmarshal(encoded, &result); err != nil {
				panic(err)
			}
			if result.Active {
				total += (result.Customer+1)*result.Amount + result.ID + int64(len(result.Note))
			}
		}
	}
	return total
}

func state(n, seed int64) int64 {
	current := make(map[int64]int64)
	versions := make([]map[int64]int64, 0)
	snapshot := func() {
		copy := make(map[int64]int64, len(current))
		for k, v := range current {
			copy[k] = v
		}
		versions = append(versions, copy)
	}
	for id := int64(0); id < n; id++ {
		current[id%1024] = (id*31 + seed) % 100000
		if (id+1)%256 == 0 {
			snapshot()
		}
	}
	snapshot()
	var total int64
	for _, version := range versions {
		for k, v := range version {
			total += (k + 1) * v
		}
	}
	return total
}

func pipeline(n, seed int64, workers int, rounds int64) int64 {
	jobs := make(chan []int64, 4)
	results := make(chan int64, 4)
	var tasks sync.WaitGroup
	tasks.Add(workers + 1)
	go func() {
		defer tasks.Done()
		defer close(jobs)
		for start := int64(0); start < n; start += 64 {
			size := min(int64(64), n-start)
			batch := make([]int64, size)
			for i := range batch {
				batch[i] = start + int64(i)
			}
			jobs <- batch
		}
	}()
	for w := 0; w < workers; w++ {
		go func() {
			defer tasks.Done()
			for batch := range jobs {
				var total int64
				for _, id := range batch {
					total += score(id, seed, rounds)
				}
				results <- total
			}
		}()
	}
	var total int64
	for b := int64(0); b < (n+63)/64; b++ {
		total += <-results
	}
	tasks.Wait()
	return total
}

func main() {
	args := os.Args[1:]
	number := func(i int) int64 {
		v, err := strconv.ParseInt(args[i], 10, 64)
		if err != nil {
			panic(err)
		}
		return v
	}
	name := args[0]
	n, seed, workers, warmups, repeats := number(1), number(2), int(number(3)), number(4), number(5)
	out := bufio.NewWriter(os.Stdout)
	fmt.Fprintln(out, "READY")
	out.Flush()
	var input string
	if name == "json" {
		data, err := os.ReadFile(args[6])
		if err != nil {
			panic(err)
		}
		input = string(data)
	}
	for iteration := int64(0); iteration < warmups+repeats; iteration++ {
		started := time.Now()
		var checksum int64
		switch name {
		case "cpu":
			checksum = cpu(n, seed)
		case "reconcile":
			checksum = reconcile(n, seed)
		case "json":
			checksum = jsonOrders(input)
		case "state":
			checksum = state(n, seed)
		case "messaging":
			checksum = pipeline(n, seed, workers, 2)
		case "pipeline":
			checksum = pipeline(n, seed, workers, 128)
		default:
			panic("unknown workload")
		}
		elapsed := time.Since(started).Nanoseconds()
		fmt.Fprintf(out, "BENCH %d %d %d\n", iteration, elapsed, checksum)
		out.Flush()
	}
}
