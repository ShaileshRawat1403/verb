import re

test_path = "app/src/test/java/com/example/VerbLogicTest.kt"
with open(test_path, "r") as f:
    content = f.read()

content = content.replace("assertTrue(result.requiresConfirmation)", "println(\"RESULT: $result\")\n        assertTrue(result.requiresConfirmation)")
with open(test_path, "w") as f:
    f.write(content)
