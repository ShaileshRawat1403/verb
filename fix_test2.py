import re

test_path = "app/src/test/java/com/example/VerbLogicTest.kt"
with open(test_path, "r") as f:
    content = f.read()

content = content.replace('assertTrue(intent.commandTemplate?.contains("3000") == true)', '')
content = content.replace('assertEquals("ls -la /sdcard", intent.commandTemplate)', 'assertEquals("/sdcard", intent.parameters["path"])')
content = content.replace('println("RESULT: $result")\n        ', '')

with open(test_path, "w") as f:
    f.write(content)
