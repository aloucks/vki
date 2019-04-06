#!/bin/bash

# Helper script for compiling test and example shaders

set -e

DIR=$(dirname ${BASH_SOURCE[0]})
FILES=$(find $DIR -name "*.glsl")

for FILE in ${FILES}; do
    NAME_GLSL=$(basename ${FILE})
    NAME_SPV=${NAME_GLSL/\.glsl/\.spv}
    STAGE="unknown"
    case $NAME_SPV in
        *".vert.spv")
            STAGE="vert";;
        *".frag.spv")
            STAGE="frag";;
        *".comp.spv")
            STAGE="comp";;
    esac
    PATH_SPV=$(dirname ${FILE})/${NAME_SPV}
    GLSL_TIME=$(stat -c %Y ${FILE})
    if [[ ! -f ${PATH_SPV} || ${GLSL_TIME} > $(stat -c %Y ${PATH_SPV}) ]]; then
        echo "Compiling: ${FILE}"
        glslc -fshader-stage=${STAGE} ${FILE} -o ${PATH_SPV}
    fi
done
