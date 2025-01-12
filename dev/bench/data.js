window.BENCHMARK_DATA = {
  "lastUpdate": 1736646769749,
  "repoUrl": "https://github.com/mattwparas/steel",
  "entries": {
    "Benchmark": [
      {
        "commit": {
          "author": {
            "name": "Roberto Vidal",
            "username": "jrvidal",
            "email": "roberto.vidal@ikumene.com"
          },
          "committer": {
            "name": "GitHub",
            "username": "web-flow",
            "email": "noreply@github.com"
          },
          "id": "785312ab654d47dd87c5a2b7a137e9fd9e644346",
          "message": "vector macro patterns (#272)",
          "timestamp": "2025-01-11T17:49:19Z",
          "url": "https://github.com/mattwparas/steel/commit/785312ab654d47dd87c5a2b7a137e9fd9e644346"
        },
        "date": 1736646769142,
        "tool": "cargo",
        "benches": [
          {
            "name": "range-big",
            "value": 108725,
            "range": "± 155",
            "unit": "ns/iter"
          },
          {
            "name": "map-big",
            "value": 634763,
            "range": "± 1280",
            "unit": "ns/iter"
          },
          {
            "name": "transducer-map",
            "value": 1715856,
            "range": "± 29560",
            "unit": "ns/iter"
          },
          {
            "name": "filter-big",
            "value": 488651,
            "range": "± 13424",
            "unit": "ns/iter"
          },
          {
            "name": "ten-thousand-iterations",
            "value": 891734,
            "range": "± 11713",
            "unit": "ns/iter"
          },
          {
            "name": "ten-thousand-iterations-letrec",
            "value": 1359464,
            "range": "± 13980",
            "unit": "ns/iter"
          },
          {
            "name": "trie-sort-without-optimizations",
            "value": 441801,
            "range": "± 2100",
            "unit": "ns/iter"
          },
          {
            "name": "fib-28/fib-28",
            "value": 69861785,
            "range": "± 1220520",
            "unit": "ns/iter"
          },
          {
            "name": "thread-creation/thread-creation",
            "value": 931951,
            "range": "± 16964",
            "unit": "ns/iter"
          },
          {
            "name": "engine-creation",
            "value": 31890020,
            "range": "± 858206",
            "unit": "ns/iter"
          },
          {
            "name": "register-fn",
            "value": 193,
            "range": "± 44",
            "unit": "ns/iter"
          },
          {
            "name": "multiple-transducers",
            "value": 9369,
            "range": "± 328",
            "unit": "ns/iter"
          },
          {
            "name": "ackermann-3-3",
            "value": 328924,
            "range": "± 7687",
            "unit": "ns/iter"
          }
        ]
      }
    ]
  }
}