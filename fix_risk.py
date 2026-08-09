import re

ar_path = "app/src/main/java/com/example/verb/actions/ActionRegistry.kt"
with open(ar_path, "r") as f:
    ar = f.read()

ar = ar.replace("if (intent.risk == ActionRisk.CONTROLLED_WRITE && !confirmed)", "if (enforcedIntent.risk == ActionRisk.CONTROLLED_WRITE && !confirmed)")
ar = ar.replace("if (intent.id == \"process.stop\")", "if (enforcedIntent.id == \"process.stop\")")
ar = ar.replace("val pidStr = intent.parameters[\"pid\"] ?: \"\"", "val pidStr = enforcedIntent.parameters[\"pid\"] ?: \"\"")
ar = ar.replace("originalIntent = intent", "originalIntent = enforcedIntent")

with open(ar_path, "w") as f: f.write(ar)
print("Risk fixed.")
