#!/usr/bin/env bash

DIR_PATH=$(realpath $(dirname "$0"))
source $DIR_PATH/colors.sh

IMAGE_NAME="auser/testkit"
IMAGE_TAG="latest"
CONTAINER_NAME="db-testkit_devcontainer-development-1"
FORCE_REBUILD_IMAGE=false
DOCKER_DIR=".devcontainer"
RUN_PRIVILEGED=false
VERBOSE="false"
declare -a MOUNTS=("$(pwd):/workspace")

# docker_service_address=$(docker network inspect kind -f "{{(index .IPAM.Config 1).Subnet}}" | cut -d '.' -f1,2,3)
# my_ip=$(ipconfig getifaddr en0)
# api_server_address="${my_ip}"

docker_instance() {
    docker ps | grep "$CONTAINER_NAME" | awk '{print $1}'
}

build_image() {
    local image_id=$(docker images --filter=reference="$IMAGE_NAME" --format "{{.ID}}")
    if [[ "$FORCE_REBUILD_IMAGE" == "true" && -n "$image_id" ]]; then
        docker rmi "$image_id"
    fi
    local cmd=(docker build) 
    cmd+=(-t "$IMAGE_NAME:$IMAGE_TAG")
    cmd+=(-f $DOCKER_DIR/Dockerfile.base)
    [[ "$FORCE_REBUILD_IMAGE" == "true" ]] && cmd+=(--no-cache)
    cmd+=($DOCKER_DIR)

    if [[ "$VERBOSE" == "true" ]]; then
        printf "${BBlack}%s" echo -e "${BBlack}-------- Docker command --------${Color_Off}"
        printf "${BBlack}%s" echo -e "${Green}${cmd[@]}${Color_Off}"
    fi

    "${cmd[@]}"

    if [[ $? -eq 0 ]]; then
        printf "${BBlack}${Green}%s${Color_Off}" "Image $IMAGE_NAME:$IMAGE_TAG built successfully"
    else
        printf "${BBlack}${Red}%s${Color_Off}" "Failed to build image $IMAGE_NAME:$IMAGE_TAG"
        exit 1
    fi

    docker tag "$IMAGE_NAME:$IMAGE_TAG" "$IMAGE_NAME:latest"
}

start_container() {
    local docker_instance=$(docker_instance)
    echo "$docker_instance"
    if [[ -z "$docker_instance" ]]; then
        local cmd=(docker run --rm -it)
        [[ "$RUN_PRIVILEGED" == "true" ]] && cmd+=(--privileged)

        # Add volume mounts to the command
        for mount in "${MOUNTS[@}"; do
            cmd+=(-v "$mount")
        done
        cmd+=($ADDITIONAL_ARGS)
        [[ -n "$CONTAINER_NAME" ]] && cmd+=(--name "$CONTAINER_NAME")

        cmd+=(--tmpfs /tmp --tmpfs /run)
        # --cpus="2.0" --memory="32g" --memory-swap=-1 --memory-reservation="16g"

        cmd+=(-d "$IMAGE_NAME" /sbin/init)

        if [[ "$VERBOSE" == "true" ]]; then
            echo_color "BBlack" "-------- Docker command --------"
            echo_color "Green" "${cmd[@}"
        fi

        # Execute the command
        "${cmd[@}"

        sleep 2
    fi
}

exec_instance() {
    local docker_instance=$(docker_instance)
    if [[ -z "$docker_instance" ]]; then
        printf "${BRed}No container found${Color_Off}"
        exit 1
    fi
    docker exec -it ${docker_instance} /usr/bin/zsh
}

parse_opts() {
    local opt
    while getopts "n:v" opt; do
        case ${opt} in
            n ) CLUSTER_NAME=$OPTARG ;;
            v ) VERBOSE="true" ;;
            \? ) echo "Invalid option: $OPTARG" 1>&2; exit 1 ;;
        esac
    done
}

help() {
    echo -e "${BGreen}Usage: $(basename "$0") [options] <command>${Color_Off}
Options:
  -n  Name of the cluster (default: $CLUSTER_NAME)
  -v  Verbose mode

Commands:
  ${Green}build${Color_Off}             Build the Docker image
  ${Green}start${Color_Off}             Start the Docker container
  ${Green}exec${Color_Off}              Exec into the container
  ${Green}reset${Color_Off}             Reset the cluster
"
    exit 1
}

main() {
    parse_opts "$@"
    shift $((OPTIND - 1))
    if [ $# -eq 0 ]; then
        help
    fi
    case "$1" in
        build) build_image ;;
        start) start_container ;;
        exec) exec_instance ;;
        reset) reset_cluster ;;
        *) help ;;
    esac
}

main "$@"
