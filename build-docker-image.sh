source ./vars.sh

docker build \
    -f Dockerfile \
    -t $imagename:$imagetag \
    $DIR