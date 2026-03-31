import java.util.concurrent.CompletableFuture;

class CompletableFutureTest {
    public static void main(String[] args) throws Exception {
        // supplyAsync + get
        CompletableFuture<String> cf1 = CompletableFuture.supplyAsync(() -> {
            return "hello";
        });
        System.out.println("cf1 = " + cf1.get());

        // completedFuture
        CompletableFuture<Integer> cf2 = CompletableFuture.completedFuture(42);
        System.out.println("cf2 = " + cf2.get());
        System.out.println("cf2 isDone = " + cf2.isDone());

        // thenApply
        CompletableFuture<String> cf3 = CompletableFuture.supplyAsync(() -> {
            return 10;
        }).thenApply(x -> {
            return "value=" + x;
        });
        System.out.println("cf3 = " + cf3.join());

        System.out.println("done");
    }
}
