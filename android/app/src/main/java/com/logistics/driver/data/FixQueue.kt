package com.logistics.driver.data

import android.content.Context
import androidx.room.ColumnInfo
import androidx.room.Dao
import androidx.room.Database
import androidx.room.Entity
import androidx.room.Insert
import androidx.room.PrimaryKey
import androidx.room.Query
import androidx.room.Room
import androidx.room.RoomDatabase
import androidx.room.Transaction
import com.logistics.driver.domain.Fix
import kotlinx.coroutines.flow.Flow

/** A fix waiting to be uploaded. [id] is local only and orders the queue, oldest first. */
@Entity(tableName = "queued_fix")
data class QueuedFix(
    @PrimaryKey(autoGenerate = true) val id: Long = 0,
    val latitude: Double,
    val longitude: Double,
    @ColumnInfo(name = "recorded_at") val recordedAt: Long,
    @ColumnInfo(name = "accuracy_m") val accuracyM: Double?,
    @ColumnInfo(name = "speed_mps") val speedMps: Double?,
) {
    fun toFix() = Fix(latitude, longitude, recordedAt, accuracyM, speedMps)

    companion object {
        fun of(fix: Fix) = QueuedFix(
            latitude = fix.latitude,
            longitude = fix.longitude,
            recordedAt = fix.recordedAt,
            accuracyM = fix.accuracyM,
            speedMps = fix.speedMps,
        )
    }
}

/** The offline queue. Survives the service or the whole process being killed. */
interface FixQueue {
    val size: Flow<Int>

    /** Append [fix]; past [cap] fixes the oldest are dropped. */
    suspend fun enqueue(fix: Fix, cap: Int)

    /** The oldest [limit] fixes, left in the queue until [remove] is called for them. */
    suspend fun oldest(limit: Int): List<QueuedFix>

    /** Remove exactly these fixes (an uploaded batch); anything queued since stays. */
    suspend fun remove(ids: List<Long>)

    suspend fun clear()
}

@Dao
abstract class FixDao {
    @Insert
    protected abstract suspend fun insert(fix: QueuedFix): Long

    @Query("DELETE FROM queued_fix WHERE id NOT IN (SELECT id FROM queued_fix ORDER BY id DESC LIMIT :keep)")
    protected abstract suspend fun keepNewest(keep: Int)

    @Transaction
    open suspend fun enqueue(fix: QueuedFix, cap: Int) {
        insert(fix)
        keepNewest(cap)
    }

    @Query("SELECT * FROM queued_fix ORDER BY id LIMIT :limit")
    abstract suspend fun oldest(limit: Int): List<QueuedFix>

    @Query("DELETE FROM queued_fix WHERE id IN (:ids)")
    abstract suspend fun deleteByIds(ids: List<Long>)

    @Query("SELECT COUNT(*) FROM queued_fix")
    abstract fun count(): Flow<Int>

    @Query("DELETE FROM queued_fix")
    abstract suspend fun clear()
}

@Database(entities = [QueuedFix::class], version = 1, exportSchema = false)
abstract class AppDb : RoomDatabase() {
    abstract fun fixDao(): FixDao

    companion object {
        fun open(context: Context): AppDb =
            Room.databaseBuilder(context.applicationContext, AppDb::class.java, "driver.db").build()
    }
}

class RoomFixQueue(private val dao: FixDao) : FixQueue {
    override val size: Flow<Int> = dao.count()
    override suspend fun enqueue(fix: Fix, cap: Int) = dao.enqueue(QueuedFix.of(fix), cap)
    override suspend fun oldest(limit: Int) = dao.oldest(limit)
    override suspend fun remove(ids: List<Long>) {
        if (ids.isNotEmpty()) dao.deleteByIds(ids)
    }
    override suspend fun clear() = dao.clear()
}
