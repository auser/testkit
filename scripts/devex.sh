#!/usr/bin/env bash

set -euo pipefail
DIR_PATH=$(realpath $(dirname "$0"))

IMAGE_NAME="auser/dbtestkit"
IMAGE_TAG="latest"
CONTAINER_NAME="db_testkit_devcontainer-development"
FORCE_REBUILD_IMAGE=false
DEVCONTAINER_DIR=".devcontainer"  
DOCKER_DIR="$DEVCONTAINER_DIR/docker"
RUN_PRIVILEGED=false
VERBOSE="false"
FORCE_RESET_CONTAINER=true
FORCE_REBUILD_IMAGE=true

declare -a MOUNTS=("$(pwd):/workspace")

export Color_Off='\033[0m'
export Black='\033[0;30m'
export Red='\033[0;31m'
export Green='\033[0;32m'
export Yellow='\033[0;33m'
export Blue='\033[0;34m'
export Purple='\033[0;35m'
export Cyan='\033[0;36m'
export White='\033[0;37m'
export BBlack='\033[1;30m'
export BRed='\033[1;31m'
export BGreen='\033[1;32m'
export BYellow='\033[1;33m'
export BBlue='\033[1;34m'
export BPurple='\033[1;35m'
export BCyan='\033[1;36m'
export BWhite='\033[1;37m'
export UBlack='\033[4;30m'
export URed='\033[4;31m'
export UGreen='\033[4;32m'
export UYellow='\033[4;33m'
export UBlue='\033[4;34m'
export UPurple='\033[4;35m'
export UCyan='\033[4;36m'
export UWhite='\033[4;37m'

DEVCONTAINER_BIN=$(which devcontainer 2>/dev/null)
if [[ -z "$DEVCONTAINER_BIN" ]]; then
    printf "${RED}Error: devcontainer CLI not found. Please install it first.${COLOR_OFF}\n"
    exit 1
fi


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
    cmd+=(-f $DOCKER_DIR/Dockerfile)
    [[ "$FORCE_REBUILD_IMAGE" == "true" ]] && cmd+=(--no-cache)
    cmd+=($DEVCONTAINER_DIR)

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
            echo_color "Green" "${cmd[@]}"
        fi

        # Execute the command
        "${cmd[@]}"

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

reset_container() {
    echo -e "${BBlack}${Yellow}Resetting container...${Color_Off}"
    ARGS=""

    echo "FORCE_REBUILD_IMAGE: $FORCE_REBUILD_IMAGE"
    echo "FORCE_RESET_CONTAINER: $FORCE_RESET_CONTAINER"
    if [[ "$FORCE_REBUILD_IMAGE" == "true" ]]; then
        ARGS="--build-no-cache"
    fi

    if [[ "$FORCE_RESET_CONTAINER" == "true" ]]; then
        ARGS="$ARGS --remove-existing-container"
    fi

    echo "$DEVCONTAINER_BIN up $ARGS"
    $DEVCONTAINER_BIN up $ARGS
}


parse_opts() {
    local opt
    while getopts "n:vfr" opt; do
        case ${opt} in
            v ) VERBOSE="true" ;;
            n ) CONTAINER_NAME=$OPTARG ;;
            f ) FORCE_REBUILD_IMAGE="false" ;;
            r ) FORCE_RESET_CONTAINER="false" ;;
            \? ) echo "Invalid option: $OPTARG" 1>&2; exit 1 ;;
        esac
    done
}

help() {
    echo -e "${BGreen}Usage: $(basename "$0") [options] <command>${Color_Off}
Options:
  -n  Name of the container (default: $CONTAINER_NAME)
  -v  Verbose mode
  -f  Do not force rebuild image
  -r  Do not force reset container

Commands:
  ${Green}build${Color_Off}             Build the Docker image
  ${Green}start${Color_Off}             Start the Docker container
  ${Green}exec${Color_Off}              Exec into the container
  ${Green}reset${Color_Off}             Reset the container
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
        reset) reset_container ;;
        *) help ;;
    esac
}

main "$@"
