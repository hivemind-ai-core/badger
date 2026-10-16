package main

import (
	"fmt"
	"os"
	"strconv"
	"time"
)

type Event struct {
	ID, Value int64
	Active    bool
}

func cpu(n, seed int64) int64 {
	var total int64
	for i := int64(0); i < n; i++ {
		x := (i + seed) % 1000003
		for r := 0; r < 8; r++ {
			x = (x*17 + 23) % 1000003
		}
		total += x
	}
	return total
}

func records(n, seed int64) int64 {
	original := make([]Event, 0, n)
	for i := int64(0); i < n; i++ {
		original = append(original, Event{i, (i + seed) % 1000, i%3 != 0})
	}
	enriched := make([]Event, 0, n)
	for _, e := range original {
		e.Value = e.Value*3 + 7
		enriched = append(enriched, e)
	}
	var total int64
	for _, e := range enriched {
		if e.Active {
			total += e.Value
		}
	}
	for _, e := range original {
		total += e.Value
	}
	return total
}

func clone(m map[int64]int64) map[int64]int64 {
	copy := make(map[int64]int64, len(m))
	for k, v := range m {
		copy[k] = v
	}
	return copy
}

func snapshots(n, seed int64) int64 {
	current := make(map[int64]int64)
	versions := []map[int64]int64{}
	for i := int64(0); i < n; i++ {
		current[i%256] = i + seed
		if (i+1)%64 == 0 {
			versions = append(versions, clone(current))
		}
	}
	versions = append(versions, clone(current))
	var total int64
	for _, version := range versions {
		for k, v := range version {
			total += (k + 1) * v
		}
	}
	return total
}

func handoff(n, seed int64, batch int) int64 {
	var total int64
	done := make(chan struct{})
	if batch == 1 {
		ch := make(chan int64, 256)
		go func() {
			defer close(done)
			defer close(ch)
			for i := int64(0); i < n; i++ {
				ch <- i + seed
			}
		}()
		for x := range ch {
			total += x
		}
	} else {
		ch := make(chan []int64, 4)
		go func() {
			defer close(done)
			defer close(ch)
			for start := int64(0); start < n; start += 64 {
				values := make([]int64, 0, 64)
				for i := start; i < start+64 && i < n; i++ {
					values = append(values, i+seed)
				}
				ch <- values
			}
		}()
		for values := range ch {
			for _, x := range values {
				total += x
			}
		}
	}
	<-done
	return total
}

func main() {
	name := os.Args[1]
	args := []int64{}
	for _, text := range os.Args[2:] {
		n, err := strconv.ParseInt(text, 10, 64)
		if err != nil {
			panic(err)
		}
		args = append(args, n)
	}
	n, seed, warmups, repeats := args[0], args[1], args[2], args[3]
	for iteration := int64(0); iteration < warmups+repeats; iteration++ {
		started := time.Now()
		var checksum int64
		switch name {
		case "cpu":
			checksum = cpu(n, seed)
		case "records":
			checksum = records(n, seed)
		case "snapshots":
			checksum = snapshots(n, seed)
		case "handoff":
			checksum = handoff(n, seed, 1)
		case "batches":
			checksum = handoff(n, seed, 64)
		default:
			panic("unknown workload")
		}
		elapsed := time.Since(started).Nanoseconds()
		fmt.Println("BENCH", iteration, elapsed, checksum)
	}
}
