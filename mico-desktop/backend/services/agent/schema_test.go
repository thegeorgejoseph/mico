package agent

import (
	"strings"
	"testing"
)

func TestOutputSchemaAvoidsUnsupportedCodexCombinators(t *testing.T) {
	schemaJSON, err := outputSchema()
	if err != nil {
		t.Fatalf("outputSchema() error = %v", err)
	}
	for _, disallowed := range []string{`"oneOf"`, `"anyOf"`, `"allOf"`} {
		if strings.Contains(schemaJSON, disallowed) {
			t.Fatalf("schema unexpectedly contains %s: %s", disallowed, schemaJSON)
		}
	}
}
