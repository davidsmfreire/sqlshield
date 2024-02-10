from typing import List


class PySqlValidationError:
    location: str
    """file and line of error"""

    description: str
    """error description"""
    
def validate(dir: str, schema_file_path: str) -> List[PySqlValidationError]:
    ...
