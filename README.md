# srchr

Unified file search for your shell: one command that matches **file names**
(via [fd](https://github.com/sharkdp/fd)) and **file contents** (via
[ripgrep](https://github.com/BurntSushi/ripgrep)), merges the results into
[fzf](https://github.com/junegunn/fzf), previews with
[bat](https://github.com/sharkdp/bat), and opens your selection in `$EDITOR`.

```
srchr <search_term>
```

- Files whose **name** matches the term and files whose **contents** match
  are combined and deduplicated into a single fzf picker.
- **Smart preview:** if the selected file contains the term, the bat preview
  jumps to the first matching line and highlights it; otherwise it shows the
  file from the top.
- **Smart open:** pressing enter opens `$EDITOR +<line> <file>` when the file
  contains the term (vim/nvim/helix-style line jump), or `$EDITOR <file>`
  otherwise.
- Content matching is smart-case (`rg -S`), mirroring fd's default.

## Requirements

[fd](https://github.com/sharkdp/fd),
[ripgrep](https://github.com/BurntSushi/ripgrep),
[fzf](https://github.com/junegunn/fzf), and
[bat](https://github.com/sharkdp/bat) on your `PATH`, plus an `$EDITOR` that
understands `+<line>` (vim, nvim, helix, kakoune, nano, ...).

## Install

### fish

Copy (or symlink) `srchr.fish` into your functions directory:

```fish
ln -s (pwd)/srchr.fish ~/.config/fish/functions/srchr.fish
```

### bash / zsh

Source `srchr.sh` from your `.bashrc` or `.zshrc`:

```sh
source /path/to/srchr/srchr.sh
```

## Notes

- The search term is passed to the fzf preview/enter commands via the
  `SRCHR_TERM` environment variable rather than string interpolation, so
  terms containing quotes or shell metacharacters are safe.
- `srchr.sh` is a single file that works in both bash and zsh.
