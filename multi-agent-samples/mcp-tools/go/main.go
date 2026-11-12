package main

import (
	"context"
	"errors"
	"log"

	"github.com/google/jsonschema-go/jsonschema"
	"github.com/modelcontextprotocol/go-sdk/mcp"
)

// Every tool answers with an object that has one field, "result".
type Number struct {
	Result float64 `json:"result"`
}

type Numbers struct {
	Result []float64 `json:"result"`
}

type MeanInput struct {
	Values []float64 `json:"values"`
}

type DotInput struct {
	A []float64 `json:"a"`
	B []float64 `json:"b"`
}

type ScaleInput struct {
	Values []float64 `json:"values"`
	Factor float64   `json:"factor"`
}

// A wrong input is an error of the tool: the SDK sends the text with isError set.
func mean(ctx context.Context, req *mcp.CallToolRequest, in MeanInput) (*mcp.CallToolResult, Number, error) {
	if len(in.Values) == 0 {
		return nil, Number{}, errors.New("values must not be empty")
	}
	sum := 0.0
	for _, v := range in.Values {
		sum += v
	}
	return nil, Number{sum / float64(len(in.Values))}, nil
}

func dot(ctx context.Context, req *mcp.CallToolRequest, in DotInput) (*mcp.CallToolResult, Number, error) {
	if len(in.A) != len(in.B) {
		return nil, Number{}, errors.New("vectors must have the same size")
	}
	if len(in.A) == 0 {
		return nil, Number{}, errors.New("values must not be empty")
	}
	sum := 0.0
	for i := range in.A {
		sum += in.A[i] * in.B[i]
	}
	return nil, Number{sum}, nil
}

func scale(ctx context.Context, req *mcp.CallToolRequest, in ScaleInput) (*mcp.CallToolResult, Numbers, error) {
	if len(in.Values) == 0 {
		return nil, Numbers{}, errors.New("values must not be empty")
	}
	out := make([]float64, len(in.Values))
	for i, v := range in.Values {
		out[i] = v * in.Factor
	}
	return nil, Numbers{out}, nil
}

// The SDK would work out the input schema from the structs above, but it describes a list as
// "null or array". These schemas say what we mean: a list of numbers, and a number.
// Each one is made new every time, because a schema cannot appear twice in a tree.
func numbers() *jsonschema.Schema {
	return &jsonschema.Schema{Type: "array", Items: &jsonschema.Schema{Type: "number"}}
}

func number() *jsonschema.Schema {
	return &jsonschema.Schema{Type: "number"}
}

func input(required []string, properties map[string]*jsonschema.Schema) *jsonschema.Schema {
	return &jsonschema.Schema{Type: "object", Properties: properties, Required: required}
}

func main() {
	// The name and the version are what this server tells Metagente about itself.
	server := mcp.NewServer(&mcp.Implementation{Name: "calc", Version: "1.0.0"}, nil)
	mcp.AddTool(server, &mcp.Tool{
		Name:        "mean",
		Description: "Average of a list of numbers.",
		InputSchema: input([]string{"values"}, map[string]*jsonschema.Schema{"values": numbers()}),
	}, mean)
	mcp.AddTool(server, &mcp.Tool{
		Name:        "dot",
		Description: "Dot product of two lists of numbers of the same size.",
		InputSchema: input([]string{"a", "b"}, map[string]*jsonschema.Schema{"a": numbers(), "b": numbers()}),
	}, dot)
	mcp.AddTool(server, &mcp.Tool{
		Name:        "scale",
		Description: "Every number of the list multiplied by the factor.",
		InputSchema: input([]string{"values", "factor"}, map[string]*jsonschema.Schema{"values": numbers(), "factor": number()}),
	}, scale)

	// Talk to Metagente through standard input and output.
	// Never print anything else to standard output (fmt.Println): it would break the conversation.
	// For messages to yourself use log, which writes to the standard error.
	if err := server.Run(context.Background(), &mcp.StdioTransport{}); err != nil {
		log.Fatal(err)
	}
}
