import java.util.concurrent.ForkJoinPool;
import java.util.concurrent.RecursiveTask;

class SumTask extends RecursiveTask<Integer> {
    int lo;
    int hi;

    SumTask(int lo, int hi) {
        this.lo = lo;
        this.hi = hi;
    }

    protected Integer compute() {
        if (hi - lo <= 2) {
            int sum = 0;
            for (int i = lo; i < hi; i++) sum += i;
            return sum;
        }
        int mid = (lo + hi) / 2;
        SumTask left = new SumTask(lo, mid);
        SumTask right = new SumTask(mid, hi);
        left.fork();
        int rightResult = right.compute();
        int leftResult = left.join();
        return leftResult + rightResult;
    }
}

class ForkJoinTest {
    public static void main(String[] args) {
        ForkJoinPool pool = new ForkJoinPool();
        SumTask task = new SumTask(0, 10);
        int result = pool.invoke(task);
        System.out.println(result);
    }
}
