from typing import List


class PySqlValidationError:
    location: str
    """file and line of error"""

    description: str
    """error description"""


def validate_files(dir: str, schema_file_path: str) -> List[PySqlValidationError]:
    ...


def validate_query(query: str, schema: str) -> List[str]:
    ...
