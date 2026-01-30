cargo new klog




The Architecture: "The Fan-In Pattern"
```
[ Pod A ] --(Stream)--> \
[ Pod B ] --(Stream)-->  [ Rust Async Channel (MPSC) ] --> [ Main Print Loop ]
[ Pod C ] --(Stream)--> /
```

To start it just run the command and rest can be read from help
