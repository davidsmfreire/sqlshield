from typing import List
from sqlshield import validate, PySqlValidationError

result: List[PySqlValidationError] = validate("../sqlshield-cli/tests/main.py", "../sqlshield-cli/tests/schema.sql")

for r in result:
    print(r.location)
    print(r.description)
