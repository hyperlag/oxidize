import java.util.concurrent.CompletableFuture;

class LambdaBlock {
    public static void main(String[] args) throws Exception {
        // Block lambda with multiple statements and return value
        CompletableFuture<String> cf = CompletableFuture.supplyAsync(() -> {
            String part1 = "hello";
            String part2 = " world";
            return part1 + part2;
        });
        System.out.println("cf = " + cf.join());

        // Block lambda with single return (already worked, regression check)
        CompletableFuture<Integer> cf2 = CompletableFuture.supplyAsync(() -> {
            return 42;
        });
        System.out.println("cf2 = " + cf2.join());

        // Block lambda with side effects and return
        CompletableFuture<String> cf3 = CompletableFuture.supplyAsync(() -> {
            int x = 5;
            int y = x * 10;
            String msg = "computed=" + y;
            return msg;
        });
        System.out.println("cf3 = " + cf3.join());

        System.out.println("done");
    }
}
