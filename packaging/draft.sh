#!/bin/sh
# Update the dev draft for this commit. A superseded dev head is left untouched.
set -eu
repo="${GITHUB_REPOSITORY:?}"
commit="${GITHUB_SHA:?}"
current=$(gh api "repos/$repo/git/ref/heads/dev" --jq .object.sha)
if [ "$current" != "$commit" ]; then
    echo "Skipping a superseded development build"
    exit 0
fi
test -f dist/mako-x86_64.AppImage
test -f dist/mako-x86_64.AppImage.sha256
test ! -e dist/mako-x86_64.AppImage.zsync
(cd dist && sha256sum -c mako-x86_64.AppImage.sha256)
payload=$(printf '%s' "{\"tag_name\":\"dev\",\"target_commitish\":\"$commit\",\"name\":\"mako dev\",\"draft\":true,\"prerelease\":true,\"body\":\"Development build. This is not a stable release and it does not update itself.\"}")
if gh api "repos/$repo/releases/tags/dev" >/dev/null 2>&1; then
    id=$(gh api "repos/$repo/releases/tags/dev" --jq .id)
    draft=$(gh api "repos/$repo/releases/tags/dev" --jq .draft)
    if [ "$draft" != "true" ]; then
        echo "The dev release is published; refusing to change it" >&2
        exit 1
    fi
    printf '%s' "$payload" | gh api -X PATCH "repos/$repo/releases/$id" --input - >/dev/null
else
    printf '%s' "$payload" | gh api -X POST "repos/$repo/releases" --input - >/dev/null
fi
gh release upload dev dist/mako-x86_64.AppImage dist/mako-x86_64.AppImage.sha256 --repo "$repo" --clobber
gh api "repos/$repo/releases/tags/dev" --jq '.assets[].name' | while read -r name; do
    case "$name" in
        mako-x86_64.AppImage|mako-x86_64.AppImage.sha256) ;;
        *)
            asset=$(gh api "repos/$repo/releases/tags/dev" --jq ".assets[] | select(.name==\"$name\") | .id")
            gh api -X DELETE "repos/$repo/releases/assets/$asset"
            ;;
    esac
done
gh api "repos/$repo/releases/tags/dev" --jq .html_url
