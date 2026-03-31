import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;
import java.util.concurrent.TimeUnit;

class AddTask implements Runnable {
    public void run() {
        System.out.println("task ran");
    }
}

class ExecutorServiceTest {
    public static void main(String[] args) throws Exception {
        ExecutorService executor = Executors.newFixedThreadPool(2);
        System.out.println("pool created");

        AddTask task = new AddTask();
        executor.execute(task);

        Thread.sleep(500);

        executor.shutdown();
        boolean terminated = executor.awaitTermination(5, TimeUnit.SECONDS);
        System.out.println("terminated = " + terminated);
        System.out.println("isShutdown = " + executor.isShutdown());
    }
}
