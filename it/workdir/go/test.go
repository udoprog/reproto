package main

import (
    "bufio"
    "fmt"
    "log"
    "os"
    "encoding/json"
)

import "./models/test"

func main() {
    scanner := bufio.NewScanner(os.Stdin)

    for scanner.Scan() {
        line := scanner.Text()
        entry := test.Entry{}

        err := json.Unmarshal([]byte(line), &entry)

        if err != nil {
            log.Fatal(err)
        }

        data, err := json.Marshal(entry)

        if err != nil {
            log.Fatal(err)
        }

        fmt.Println(string(data))
    }
}
