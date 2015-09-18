## Useful links

* tutorial on using FUSE filesystem in rust [part I](http://zsiciarz.github.io/24daysofrust/book/day15.html) and [part II](http://zsiciarz.github.io/24daysofrust/book/day16.html) and related [source code](https://github.com/zsiciarz/24daysofrust/blob/master/src/day15.rs);
* [tempfile library](https://www.reddit.com/r/rust/comments/32n864/tempfile_temporary_file_library/);
* how to restore duplicity backups in the [worst case](https://wiki.gnome.org/Apps/DejaDup/Help/Restore/WorstCase);
* [man rdiff](http://linux.die.net/man/1/rdiff).

## Duplicity

To restore a backup without passphrase into a directory use this command:

```
duplicity restore --no-encryption file://<absolute-path-of-backup> <path-to-restore>
```

To backup incrementally from an existing backup and a source directory:

```
duplicity incremental --no-encryption <source-dir> file://<absolute-path-of-backup>
```

To list current files in backup:

```
duplicity list-current-files --no-encryption file://<absolute-path-of-backup>
```

To list all the backup snapshots contained in a directory:

```
duplicity collection-status --no-encryption file://<absolute-path-of-backup>
```

## Duplicity file format

### Filenames

File extensions are used to determine if the file is compressed or encrypted:

* `.gz` is a compressed file;
* `.gpg` is an encrypted file.

File names are used to define the type, the time, the volume and the relationships between snapshots. Those files must obey certain regular expressions to be considered part of a duplicity backup.


### Signatures

A signature file is a tar file (compressed and/or encrypted) with a defined structure.
Every file in the tar is in one of the following folders:

* `signature`;
* `snapshot`;
* `deleted`.

Any other folder is ignored.
