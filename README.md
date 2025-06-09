# smartdiff

This is a command-line tool to compare and highlight differences between versions of rooms in Super Metroid SMART projects. It will compare the working copy against a selected git reference (e.g., branch, tag, or commit ID). Download the latest version from the [Releases](https://github.com/blkerby/smartdiff/releases) page.

Here are example ways to use it:

- Compare against HEAD: 

  ```smartdiff```

- Compare against local branch: 

  ```smartdiff mybranch```

- Compare against remote branch: 

  ```smartdiff origin/mybranch```

Keyboard shortcuts:
- `=`/`-`: Zoom in/out
- `1`: Toggle showing layer 1
- `2`: Toggle showing layer 2
- `t`: Toggle highlight transparency in pink (vs. black)
- `w`: Show working copy
- `r`: Show git reference
- `d`: Show difference mask between working copy and git reference

