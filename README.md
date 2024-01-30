# SQL Shield

Validate raw SQL queries present in your Python codebase against a schema using ```sqlshield```:

```shell
$ sqlshield --help
Usage: sqlshield [OPTIONS]

Options:
  -d, --directory <DIRECTORY>
          Directory. Defaults to "." (current)

  -s, --schema <SCHEMA>
          Schema file. Defaults to "schema.sql"

  -h, --help
          Print help (see a summary with '-h')

  -V, --version
          Print version
```

## Features

The tool validates the following main clauses:

- SELECT :heavy_check_mark:
  - WITH :heavy_check_mark:
  - JOIN :heavy_check_mark:
- INSERT :x:
- UPDATE :x:
- DELETE :x:

Other clauses:

- WHERE :x:
- ORDER BY :x:
- GROUP BY :x:
- HAVING :x:

## Similar work

- <https://github.com/andywer/postguard>
- <https://github.com/schemasafe>
