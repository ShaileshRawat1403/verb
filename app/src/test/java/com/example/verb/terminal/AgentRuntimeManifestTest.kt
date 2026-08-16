package com.example.verb.terminal

import org.junit.Assert.assertTrue
import org.junit.Test
import java.io.File

class AgentRuntimeManifestTest {
    private fun manifest(
        architecture: String = "aarch64",
        digest: String = "a".repeat(64),
        required: List<String> = listOf("/bin/bash", "/usr/bin/node", "/usr/bin/npm")
    ) = AgentRuntimeManifest(
        runtimeVersion = "1.0.0",
        architecture = architecture,
        rootfsSha256 = digest,
        distro = "debian-arm64",
        nodeVersion = "24.18.0",
        claudeVersion = "2.1.233",
        openCodeVersion = "1.18.18",
        minimumVerbVersion = "1.0.0",
        createdAt = "2026-08-16T00:00:00Z",
        requiredCommands = required
    )

    @Test fun validArm64ManifestIsAccepted() {
        assertTrue(manifest().validateForArm64().isSuccess)
    }

    @Test fun nonArm64ManifestIsRejected() {
        assertTrue(manifest(architecture = "x86_64").validateForArm64().isFailure)
    }

    @Test fun malformedDigestIsRejected() {
        assertTrue(manifest(digest = "not-a-digest").validateForArm64().isFailure)
    }

    @Test fun relativeCommandPathIsRejected() {
        assertTrue(manifest(required = listOf("bin/bash")).validateForArm64().isFailure)
    }

    @Test fun propertiesRoundTrip() {
        val file = File.createTempFile("verb-agent-manifest", ".txt")
        try {
            file.writeText(manifest().toPropertiesText())
            assertTrue(AgentRuntimeManifest.fromFile(file).getOrThrow() == manifest())
        } finally {
            file.delete()
        }
    }

    @Test fun malformedPropertiesAreRejected() {
        val file = File.createTempFile("verb-agent-manifest", ".txt")
        try {
            file.writeText("runtimeVersion=1.0.0\narchitecture=aarch64\n")
            assertTrue(AgentRuntimeManifest.fromFile(file).isFailure)
        } finally {
            file.delete()
        }
    }
}
