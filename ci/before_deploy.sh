# This script takes care of building your crate and packaging it for release

set -ex

main() {
    local src=$(pwd) \
          stage=

    case $TRAVIS_OS_NAME in
        linux)
            stage=$(mktemp -d)
            ;;
        osx)
            stage=$(mktemp -d -t tmp)
            ;;
    esac

    test -f Cargo.lock || cargo generate-lockfile

    # TODO Update this to build the artifacts that matter to you
    cross rustc --bin ltc-modulate --target $TARGET --release -- -C lto

    # TODO Update this to package the right artifacts
    mkdir $stage/$TRAVIS_TAG
    ls target || true
    ls target/$TARGET || true
    ls target/$TARGET/release || true
    cp target/$TARGET/release/ltc-modulate $stage/$TRAVIS_TAG/ || cp target/$TARGET/release/ltc-modulate.exe $stage/$TRAVIS_TAG/

    cd $stage
    tar czf $src/$CRATE_NAME-$TRAVIS_TAG-$TARGET.tar.gz *
    cd $src

    rm -rf $stage
}

main
