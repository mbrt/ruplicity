#! /bin/bash

if [ "$#" -ne 1 ]; then
    echo "Usage: $0 <backup-directory>"
	exit 1
fi

BACKUP_DIR=$1
OUT_DIR=$BACKUP_DIR/signatures

mkdir -p $OUT_DIR
for SIGFILE in $BACKUP_DIR/*.sigtar.gz; do
	SIGFILE_OUT=`basename $SIGFILE .sigtar.gz`
	mkdir -p $OUT_DIR/$SIGFILE_OUT
	tar xf $SIGFILE -C $OUT_DIR/$SIGFILE_OUT
done
