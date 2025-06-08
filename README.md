# smartdiff

This is a tool to compare and highlight differences between versions of rooms in Super Metroid SMART projects. It will compare the working copy against a selected git reference. Example invocations:

- Compare against HEAD: 

  ```smartdiff```

- Compare against local branch: 

  ```smartdiff refs/heads/mybranch```

- Compare against remote branch: 

  ```smartdiff refs/remotes/origin/mybranch```

Keyboard shortcuts:
- `=`/`-`: Zoom in/out
- `1`: Toggle showing layer 1
- `2`: Toggle showing layer 2
- `t`: Toggle highlight transparency in pink (vs. black)
- `w`: Show working copy
- `r`: Show git reference
- `d`: Show difference mask between working copy and git reference

