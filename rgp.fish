function rgp
    set -l search_term $argv[1]

    if test -z "$search_term"
        echo "Usage: rgp <search_term>"

        return 1
    end

    rg -l $search_term | fzf --preview 'bat --color always {}' --bind 'enter:become("$EDITOR" {+})'
end
