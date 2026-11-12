package calc;

import io.modelcontextprotocol.json.McpJsonDefaults;
import io.modelcontextprotocol.server.McpServer;
import io.modelcontextprotocol.server.transport.StdioServerTransportProvider;
import io.modelcontextprotocol.spec.McpSchema.CallToolRequest;
import io.modelcontextprotocol.spec.McpSchema.CallToolResult;
import io.modelcontextprotocol.spec.McpSchema.ServerCapabilities;
import io.modelcontextprotocol.spec.McpSchema.Tool;
import java.util.ArrayList;
import java.util.List;
import java.util.Map;

public class Server {

    // The input schemas: a list of numbers, and a number.
    static Map<String, Object> numbers() {
        return Map.of("type", "array", "items", Map.of("type", "number"));
    }

    static Map<String, Object> number() {
        return Map.of("type", "number");
    }

    static Map<String, Object> input(List<String> required, Map<String, Object> properties) {
        return Map.of("type", "object", "properties", properties, "required", required);
    }

    // The arguments arrive as plain Java values: a list of numbers is a List of Number.
    static List<Double> list(CallToolRequest request, String name) {
        List<Double> values = new ArrayList<>();
        for (Object value : (List<?>) request.arguments().get(name)) {
            values.add(((Number) value).doubleValue());
        }
        return values;
    }

    // Every tool answers with an object that has one field, "result".
    // It is sent twice: as structuredContent, and as the text of one text block.
    static CallToolResult answer(Object result) throws Exception {
        Map<String, Object> object = Map.of("result", result);
        return CallToolResult.builder()
                .addTextContent(McpJsonDefaults.getMapper().writeValueAsString(object))
                .structuredContent(object)
                .build();
    }

    // A wrong input is an error of the tool: the text, with isError set.
    static CallToolResult failure(String message) {
        return CallToolResult.builder().addTextContent(message).isError(true).build();
    }

    static CallToolResult mean(CallToolRequest request) throws Exception {
        List<Double> values = list(request, "values");
        if (values.isEmpty()) return failure("values must not be empty");
        double sum = 0;
        for (double v : values) sum += v;
        return answer(sum / values.size());
    }

    static CallToolResult dot(CallToolRequest request) throws Exception {
        List<Double> a = list(request, "a");
        List<Double> b = list(request, "b");
        if (a.size() != b.size()) return failure("vectors must have the same size");
        if (a.isEmpty()) return failure("values must not be empty");
        double sum = 0;
        for (int i = 0; i < a.size(); i++) sum += a.get(i) * b.get(i);
        return answer(sum);
    }

    static CallToolResult scale(CallToolRequest request) throws Exception {
        List<Double> values = list(request, "values");
        double factor = ((Number) request.arguments().get("factor")).doubleValue();
        if (values.isEmpty()) return failure("values must not be empty");
        List<Double> scaled = new ArrayList<>();
        for (double v : values) scaled.add(v * factor);
        return answer(scaled);
    }

    public static void main(String[] args) {
        // Talk to Metagente through standard input and output.
        // Never print anything else to System.out: it would break the conversation.
        var transport = new StdioServerTransportProvider(McpJsonDefaults.getMapper());

        // The name and the version are what this server tells Metagente about itself.
        McpServer.sync(transport)
                .serverInfo("calc", "1.0.0")
                .capabilities(ServerCapabilities.builder().tools(false).build())
                .toolCall(
                        Tool.builder("mean", input(List.of("values"), Map.of("values", numbers())))
                                .description("Average of a list of numbers.").build(),
                        (exchange, request) -> {
                            try { return mean(request); } catch (Exception e) { return failure(e.toString()); }
                        })
                .toolCall(
                        Tool.builder("dot", input(List.of("a", "b"), Map.of("a", numbers(), "b", numbers())))
                                .description("Dot product of two lists of numbers of the same size.").build(),
                        (exchange, request) -> {
                            try { return dot(request); } catch (Exception e) { return failure(e.toString()); }
                        })
                .toolCall(
                        Tool.builder("scale", input(List.of("values", "factor"), Map.of("values", numbers(), "factor", number())))
                                .description("Every number of the list multiplied by the factor.").build(),
                        (exchange, request) -> {
                            try { return scale(request); } catch (Exception e) { return failure(e.toString()); }
                        })
                .build();
    }
}
