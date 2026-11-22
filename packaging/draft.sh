#!/bin/sh
# Update the dev draft for this commit. A superseded dev head is left untouched.
# Drafts are omitted by the get-release-by-tag endpoint, so this lists releases.
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
found=$(gh api --paginate "repos/$repo/releases?per_page=100" --jq '.[] | select(.tag_name=="dev") | "\(.id) \(.draft)"')
id=$(printf '%s\n' "$found" | awk 'NR==1 { print $1 }')
draft=$(printf '%s\n' "$found" | awk 'NR==1 { print $2 }')
if [ -n "$id" ] && [ "$draft" != "true" ]; then
    echo "The dev release is published; refusing to change it" >&2
    exit 1
fi
payload=$(printf '%s' "{\"tag_name\":\"dev\",\"target_commitish\":\"$commit\",\"name\":\"mako dev\",\"draft\":true,\"prerelease\":true,\"body\":\"Development build. This is not a stable release and it does not update itself.\"}")
if [ -n "$id" ]; then
    printf '%s' "$payload" | gh api -X PATCH "repos/$repo/releases/$id" --input - >/dev/null
else
    id=$(printf '%s' "$payload" | gh api -X POST "repos/$repo/releases" --input - --jq .id)
fi
gh release upload dev dist/mako-x86_64.AppImage dist/mako-x86_64.AppImage.sha256 --repo "$repo" --clobber
extra=$(gh api "repos/$repo/releases/$id/assets" --jq '.[] | select(.name != "mako-x86_64.AppImage" and .name != "mako-x86_64.AppImage.sha256") | .id')
for asset in $extra; do
    gh api -X DELETE "repos/$repo/releases/assets/$asset"
done
gh api "repos/$repo/releases/$id" --jq .html_url
