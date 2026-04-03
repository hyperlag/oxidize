import java.net.http.HttpClient;
import java.net.http.HttpRequest;
import java.net.URI;

public class HttpClientTest {
    public static void main(String[] args) {
        HttpClient client = HttpClient.newHttpClient();
        System.out.println("client created");

        HttpRequest req = HttpRequest.newBuilder()
            .uri(URI.create("http://127.0.0.1:9999/"))
            .GET()
            .build();

        System.out.println("request built");
        System.out.println("method=" + req.method());
    }
}
