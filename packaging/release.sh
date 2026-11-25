#!/bin/sh
# Publish the stable AppImage already built in dist/. VERSION must be X.Y.0.
set -eu
version=$(tr -d '[:space:]' < VERSION)
tag="v$version"
case "$version" in
    *.*.0) ;;
    *) echo "Refusing to publish $version; stable versions end in .0" >&2; exit 1 ;;
esac
test -f dist/mako-x86_64.AppImage
test -f dist/mako-x86_64.AppImage.zsync
(cd dist && sha256sum -c mako-x86_64.AppImage.sha256)
gh release create "$tag" dist/mako-x86_64.AppImage dist/mako-x86_64.AppImage.zsync dist/mako-x86_64.AppImage.sha256 \
    --repo "$GITHUB_REPOSITORY" --latest --title "mako $version" --notes "Stable release $version."
# The next stable line. The workflow token does not start another release run.
major=${version%%.*}
rest=${version#*.}
minor=${rest%%.*}
next="$major.$((minor + 1)).0"
printf '%s\n' "$next" > VERSION
git config user.name "github-actions[bot]"
git config user.email "41898282+github-actions[bot]@users.noreply.github.com"
git add VERSION
git commit -m "Add version $next"
git push origin HEAD:main
git fetch origin dev
git checkout -B dev origin/dev
git merge origin/main -m "Merge branch 'main' into dev"
git push origin dev
